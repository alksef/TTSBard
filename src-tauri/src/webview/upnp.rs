//! UPnP port forwarding module
//!
//! Provides automatic port forwarding on UPnP-enabled routers.
//!
//! Router I/O is blocking by nature (SSDP discovery, local-address probe and SOAP
//! calls), so it never runs on an async worker: every request is executed on the
//! blocking pool behind a bounded wait. Requests are numbered by generation, and
//! an operation whose generation was superseded is compensated — a mapping that
//! is confirmed after stop/disable/restart is removed instead of surviving it.

use igd::{search_gateway, Gateway, PortMappingProtocol, SearchOptions};
use netdev::Interface;
use std::net::{IpAddr, Ipv4Addr, SocketAddr, SocketAddrV4};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

/// Бюджет ожидания открытия mapping (discovery gateway + add_port).
pub const UPNP_FORWARD_TIMEOUT: Duration = Duration::from_secs(5);

/// Бюджет ожидания закрытия mapping.
pub const UPNP_REMOVE_TIMEOUT: Duration = Duration::from_secs(3);

/// Lease открытого mapping; после его истечения router снимает mapping сам.
const UPNP_LEASE_SECONDS: u32 = 3600;

/// Общий бюджет обнаружения gateway (SSDP) для одного вызова.
///
/// Ограничивает суммарное ожидание ответов SSDP и планирование кандидатов по
/// всем активным IPv4-интерфейсам. Это не общий HTTP-deadline: после ответа на
/// M-SEARCH `igd` сам выполняет HTTP GET description/control, и его
/// длительность этим бюджетом не ограничивается. Бюджет сознательно больше
/// ожидания открытия на стороне async-вызова (`UPNP_FORWARD_TIMEOUT`, 5s):
/// поздний успех уже обрабатывается lifecycle-логикой, поэтому тихо обрывать
/// перебор после четырёх секунд нельзя.
const DISCOVERY_BUDGET: Duration = Duration::from_secs(10);

/// Ожидание приёма SSDP на одном кандидате.
///
/// `igd` отправляет M-SEARCH с `MX: 3`, поэтому router может ответить почти
/// через три секунды; 3.5s покрывают это окно, а `DISCOVERY_BUDGET` ограничивает
/// серию попыток. Это ожидание bounded только на приём SSDP, не на HTTP.
const DISCOVERY_PER_CANDIDATE: Duration = Duration::from_millis(3500);

/// Причина, по которой mapping не подтверждён.
///
/// Код уходит в IPC, чтобы UI показал локализованный текст; `detail` остаётся
/// для логов (сообщения `igd` не показываются пользователю).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UpnpFailure {
    /// Router с UPnP не найден (discovery не ответил).
    GatewayUnavailable,
    /// Router отклонил `add_port`/`remove_port`.
    RouterRejected,
    /// Операция не уложилась в бюджет ожидания.
    Timeout,
    /// Намерение перехвачено более новым запросом.
    Superseded,
    /// Ошибка blocking-задачи или внутреннего состояния.
    TaskFailed,
}

impl UpnpFailure {
    /// Код для IPC и локализации.
    pub fn code(self) -> &'static str {
        match self {
            Self::GatewayUnavailable => "webview.upnp.gateway_unavailable",
            Self::RouterRejected => "webview.upnp.router_rejected",
            Self::Timeout => "webview.upnp.timeout",
            Self::Superseded => "webview.upnp.superseded",
            Self::TaskFailed => "webview.upnp.task_failed",
        }
    }
}

/// Отказ router-операции: код для UI и диагностический текст для логов.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UpnpError {
    pub failure: UpnpFailure,
    pub detail: String,
}

impl UpnpError {
    pub fn new(failure: UpnpFailure, detail: impl Into<String>) -> Self {
        Self {
            failure,
            detail: detail.into(),
        }
    }
}

impl std::fmt::Display for UpnpError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{} ({})", self.detail, self.failure.code())
    }
}

impl std::error::Error for UpnpError {}

/// Кандидат на SSDP-discovery: активный IPv4-адрес интерфейса.
#[derive(Debug, Clone, PartialEq, Eq)]
struct DiscoveryCandidate {
    /// Системное имя интерфейса — только для диагностики.
    name: String,
    /// Адрес интерфейса; используется как явный bind для UDP-socket.
    addr: Ipv4Addr,
    /// IPv4 default gateway интерфейса, если известен. Влияет только на
    /// приоритет кандидата, но не на его допустимость.
    gateway: Option<Ipv4Addr>,
}

/// Отсеивает адреса, непригодные для bind SSDP-socket.
///
/// Исключаются unspecified, loopback, multicast, broadcast и link-local
/// (`169.254.0.0/16`) адреса. Адреса RFC1918 допустимы.
fn is_discovery_addr(addr: Ipv4Addr) -> bool {
    !addr.is_unspecified()
        && !addr.is_loopback()
        && !addr.is_multicast()
        && !addr.is_broadcast()
        && !addr.is_link_local()
}

/// Выбирает активные IPv4-интерфейсы и их адреса для явного SSDP-bind.
///
/// Порядок детерминирован: сначала кандидаты с IPv4 default gateway, затем
/// остальные; внутри группы — по адресу. Дубликаты адресов удаляются. Имена и
/// тип адаптера не учитываются, RFC1918 не требуется.
fn eligible_candidates(interfaces: &[Interface]) -> Vec<DiscoveryCandidate> {
    let mut candidates = Vec::new();
    for interface in interfaces {
        if !interface.is_up() || interface.is_loopback() {
            continue;
        }

        let gateway = interface.gateway.as_ref().and_then(|device| {
            device
                .ipv4
                .iter()
                .copied()
                .find(|addr| is_discovery_addr(*addr))
        });

        for net in &interface.ipv4 {
            let addr = net.addr();
            if is_discovery_addr(addr) {
                candidates.push(DiscoveryCandidate {
                    name: interface.name.clone(),
                    addr,
                    gateway,
                });
            }
        }
    }

    candidates.sort_by_key(|candidate| (candidate.gateway.is_none(), candidate.addr));

    // Адрес может встретиться в обеих группах приоритета, поэтому сортировка не
    // делает дубликаты соседними. Удаляем повторы глобально, сохраняя первое
    // вхождение: после сортировки это gateway-вариант, если он есть.
    let mut seen = std::collections::HashSet::new();
    candidates.retain(|candidate| seen.insert(candidate.addr));

    candidates
}

/// Остаток ожидания для следующего кандидата или `None`, если бюджет исчерпан.
fn next_candidate_wait(
    elapsed: Duration,
    budget: Duration,
    per_candidate: Duration,
) -> Option<Duration> {
    let remaining = budget.checked_sub(elapsed)?;
    if remaining.is_zero() {
        None
    } else {
        Some(per_candidate.min(remaining))
    }
}

/// Перебирает кандидатов, пока SSDP не вернёт gateway.
///
/// `search` выполняет реальный поиск с явным bind на адрес кандидата и заданным
/// ожиданием приёма. Сбой одного кандидата не прерывает перебор: следующий
/// кандидат получает остаток общего бюджета. Если gateway не найден нигде,
/// возвращается ошибка с диагностикой по каждому кандидату.
fn discover_over_candidates<F>(
    candidates: &[DiscoveryCandidate],
    budget: Duration,
    per_candidate: Duration,
    mut search: F,
) -> Result<(Gateway, DiscoveryCandidate), UpnpError>
where
    F: FnMut(&DiscoveryCandidate, Duration) -> Result<Gateway, String>,
{
    if candidates.is_empty() {
        return Err(UpnpError::new(
            UpnpFailure::GatewayUnavailable,
            "no eligible IPv4 interfaces for UPnP SSDP discovery",
        ));
    }

    let started = Instant::now();
    let mut failures = Vec::new();

    for candidate in candidates {
        let Some(wait) = next_candidate_wait(started.elapsed(), budget, per_candidate) else {
            failures.push(format!(
                "discovery budget of {:?} exhausted before interface {} ({})",
                budget, candidate.name, candidate.addr
            ));
            break;
        };

        tracing::info!(
            interface = %candidate.name,
            interface_addr = %candidate.addr,
            interface_gateway = candidate
                .gateway
                .map(|gateway| gateway.to_string())
                .unwrap_or_else(|| "none".to_string()),
            wait_ms = wait.as_millis(),
            "Trying UPnP SSDP discovery on interface"
        );

        match search(candidate, wait) {
            Ok(gateway) => return Ok((gateway, candidate.clone())),
            Err(error) => {
                tracing::warn!(
                    interface = %candidate.name,
                    interface_addr = %candidate.addr,
                    error = %error,
                    "UPnP SSDP discovery failed on interface"
                );
                failures.push(format!(
                    "{} ({}): {}",
                    candidate.name, candidate.addr, error
                ));
            }
        }
    }

    Err(UpnpError::new(
        UpnpFailure::GatewayUnavailable,
        format!(
            "UPnP gateway not found on any interface: {}",
            failures.join("; ")
        ),
    ))
}

/// Router I/O для одного порта.
///
/// Реализации блокирующие и вызываются только из blocking pool. Trait существует
/// как шов: тесты подставляют искусственно медленный или отказывающий adapter.
pub trait RouterPortMapper: Send + Sync {
    /// Открыть внешний порт на router.
    fn open(&self, port: u16) -> Result<(), UpnpError>;

    /// Закрыть внешний порт на router.
    fn close(&self, port: u16) -> Result<(), UpnpError>;
}

/// Реальный adapter поверх `igd`: discovery gateway, локальный адрес и SOAP.
pub struct IgdPortMapper {
    gateway: Mutex<Option<Gateway>>,
}

impl IgdPortMapper {
    pub fn new() -> Self {
        Self {
            gateway: Mutex::new(None),
        }
    }

    /// Discover UPnP gateway on the local network
    ///
    /// Перебирает активные IPv4-интерфейсы с явным bind. Ожидание на один
    /// интерфейс и общий бюджет серии заданы `DISCOVERY_*`; HTTP, который `igd`
    /// выполняет после SSDP-ответа, этими таймаутами не покрыт.
    fn discover_gateway(&self) -> Result<(), UpnpError> {
        let mut gw = self.gateway.lock().map_err(|e| {
            UpnpError::new(
                UpnpFailure::TaskFailed,
                format!("Failed to lock gateway: {}", e),
            )
        })?;

        if gw.is_some() {
            return Ok(());
        }

        let candidates = eligible_candidates(&netdev::get_interfaces());
        tracing::info!(
            candidate_count = candidates.len(),
            "Searching for UPnP gateway"
        );

        // Явный bind на адрес кандидата: wildcard discovery не повторяется.
        let search = |candidate: &DiscoveryCandidate, wait: Duration| {
            search_gateway(SearchOptions {
                bind_addr: SocketAddr::V4(SocketAddrV4::new(candidate.addr, 0)),
                timeout: Some(wait),
                ..Default::default()
            })
            .map_err(|e| e.to_string())
        };

        let (gateway, candidate) = discover_over_candidates(
            &candidates,
            DISCOVERY_BUDGET,
            DISCOVERY_PER_CANDIDATE,
            search,
        )
        .map_err(|e| {
            tracing::warn!(error = %e, "UPnP gateway search failed");
            e
        })?;

        tracing::info!(
            gateway_addr = %gateway.addr,
            interface = %candidate.name,
            interface_addr = %candidate.addr,
            "UPnP gateway found"
        );
        *gw = Some(gateway);
        Ok(())
    }

    /// IPv4-адрес обнаруженного gateway, если он уже закэширован.
    ///
    /// Блокировка берётся ровно на время чтения и не удерживается во время
    /// сетевых вызовов, поэтому вложенных gateway lock'ов нет.
    fn cached_gateway_ip(&self) -> Result<Ipv4Addr, UpnpError> {
        let gw = self.gateway.lock().map_err(|e| {
            UpnpError::new(
                UpnpFailure::TaskFailed,
                format!("Failed to lock gateway: {}", e),
            )
        })?;
        gw.as_ref()
            .map(|gateway| *gateway.addr.ip())
            .ok_or_else(|| {
                UpnpError::new(
                    UpnpFailure::GatewayUnavailable,
                    "UPnP gateway is not available",
                )
            })
    }

    /// Get local IP address that can reach the gateway
    ///
    /// Выполняет UDP route lookup до обнаруженного gateway: соединение не
    /// отправляет пакеты, но заставляет ОС выбрать адрес интерфейса, через
    /// который gateway реально достижим. Это заменяет прежнюю проверку по
    /// 8.8.8.8, которая могла выбрать другой интерфейс.
    fn get_local_ip(&self, gateway_ip: Ipv4Addr) -> Result<Ipv4Addr, UpnpError> {
        let socket = std::net::UdpSocket::bind("0.0.0.0:0").map_err(|e| {
            UpnpError::new(
                UpnpFailure::GatewayUnavailable,
                format!("Failed to bind UDP socket: {}", e),
            )
        })?;

        socket
            .connect(SocketAddrV4::new(gateway_ip, 1900))
            .map_err(|e| {
                UpnpError::new(
                    UpnpFailure::GatewayUnavailable,
                    format!("Failed to route to gateway {}: {}", gateway_ip, e),
                )
            })?;

        let local_addr = socket.local_addr().map_err(|e| {
            UpnpError::new(
                UpnpFailure::GatewayUnavailable,
                format!("Failed to get local address: {}", e),
            )
        })?;

        match local_addr.ip() {
            IpAddr::V4(ip) => Ok(ip),
            IpAddr::V6(_) => Err(UpnpError::new(
                UpnpFailure::GatewayUnavailable,
                "Got IPv6 address, expected IPv4",
            )),
        }
    }
}

impl Default for IgdPortMapper {
    fn default() -> Self {
        Self::new()
    }
}

impl RouterPortMapper for IgdPortMapper {
    fn open(&self, port: u16) -> Result<(), UpnpError> {
        // Гарантирует закэшированный gateway; при наличии он возвращается сразу.
        self.discover_gateway()?;

        // Локальный адрес выводится из маршрута до gateway; gateway lock здесь
        // не удерживается, чтобы не было вложенных блокировок.
        let gateway_ip = self.cached_gateway_ip()?;
        let local_ip = self.get_local_ip(gateway_ip)?;

        // Now lock gateway for the add_port call
        let gw = self.gateway.lock().map_err(|e| {
            UpnpError::new(
                UpnpFailure::TaskFailed,
                format!("Failed to lock gateway: {}", e),
            )
        })?;

        let gateway = gw.as_ref().ok_or_else(|| {
            UpnpError::new(
                UpnpFailure::GatewayUnavailable,
                "UPnP gateway is not available",
            )
        })?;
        let local_addr = SocketAddrV4::new(local_ip, port);

        tracing::info!(
            external_port = port,
            local_addr = %local_addr,
            "Adding UPnP port mapping"
        );

        gateway
            .add_port(
                PortMappingProtocol::TCP,
                port,
                local_addr,
                UPNP_LEASE_SECONDS,
                "ttsbard-webview",
            )
            .map_err(|e| {
                tracing::warn!(error = %e, "Failed to add UPnP port mapping");
                UpnpError::new(
                    UpnpFailure::RouterRejected,
                    format!("Failed to add port mapping: {}", e),
                )
            })?;

        tracing::info!(port, "UPnP port forwarding enabled");
        Ok(())
    }

    fn close(&self, port: u16) -> Result<(), UpnpError> {
        let gw = self.gateway.lock().map_err(|e| {
            UpnpError::new(
                UpnpFailure::TaskFailed,
                format!("Failed to lock gateway: {}", e),
            )
        })?;

        // Mapping мог быть открыт только после успешного discovery: без
        // кэшированного gateway закрывать нечего.
        let Some(gateway) = gw.as_ref() else {
            return Ok(());
        };

        tracing::debug!(port, "Removing UPnP port mapping");
        gateway
            .remove_port(PortMappingProtocol::TCP, port)
            .map_err(|e| {
                tracing::warn!(error = %e, port, "Failed to remove UPnP port mapping");
                UpnpError::new(
                    UpnpFailure::RouterRejected,
                    format!("Failed to remove port mapping: {}", e),
                )
            })?;

        tracing::info!(port, "UPnP port mapping removed");
        Ok(())
    }
}

/// UPnP manager for automatic port forwarding
///
/// Владеет желаемым состоянием forwarding и подтверждённым mapping. Каждый вызов
/// получает новое поколение: результат устаревшего поколения не подтверждается,
/// а фактическое состояние router приводится к последнему желаемому, поэтому
/// поздний mapping не переживает stop/disable.
pub struct UpnpManager {
    port: u16,
    mapper: Arc<dyn RouterPortMapper>,
    forward_timeout: Duration,
    remove_timeout: Duration,
    desired: AtomicBool,
    mapping_open: AtomicBool,
    epoch: AtomicU64,
}

impl UpnpManager {
    /// Create a new UPnP manager for the given port
    pub fn new(port: u16) -> Self {
        Self::with_mapper(port, Arc::new(IgdPortMapper::new()))
    }

    /// Менеджер с подставленным adapter'ом (тесты и альтернативные реализации
    /// router I/O).
    pub fn with_mapper(port: u16, mapper: Arc<dyn RouterPortMapper>) -> Self {
        Self::with_mapper_and_timeouts(port, mapper, UPNP_FORWARD_TIMEOUT, UPNP_REMOVE_TIMEOUT)
    }

    /// Менеджер с явными бюджетами ожидания: тесты укорачивают их, чтобы
    /// детерминированно проверять timeout и компенсацию.
    pub fn with_mapper_and_timeouts(
        port: u16,
        mapper: Arc<dyn RouterPortMapper>,
        forward_timeout: Duration,
        remove_timeout: Duration,
    ) -> Self {
        tracing::info!(port, "UPnP manager created");
        Self {
            port,
            mapper,
            forward_timeout,
            remove_timeout,
            desired: AtomicBool::new(false),
            mapping_open: AtomicBool::new(false),
            epoch: AtomicU64::new(0),
        }
    }

    /// Желаемое состояние forwarding (последнее запрошенное).
    pub fn is_desired(&self) -> bool {
        self.desired.load(Ordering::SeqCst)
    }

    /// Подтверждённый mapping: router сообщил об успехе, и намерение не менялось.
    pub fn is_mapping_open(&self) -> bool {
        self.mapping_open.load(Ordering::SeqCst)
    }

    fn budget(&self, enabled: bool) -> Duration {
        if enabled {
            self.forward_timeout
        } else {
            self.remove_timeout
        }
    }

    fn is_current(&self, epoch: u64) -> bool {
        self.epoch.load(Ordering::SeqCst) == epoch
    }

    /// Идемпотентно приводит router к желаемому состоянию.
    ///
    /// Async lifecycle не блокируется: router I/O идёт на blocking pool с
    /// ограниченным ожиданием. Результат, чьё поколение уже перехвачено более
    /// новым запросом, не подтверждается и не оставляет нежелательный mapping.
    pub async fn set_enabled(self: &Arc<Self>, enabled: bool) -> Result<(), UpnpError> {
        let epoch = self.epoch.fetch_add(1, Ordering::SeqCst) + 1;
        self.desired.store(enabled, Ordering::SeqCst);

        let outcome = self.run_router_operation(epoch, enabled).await;

        if !self.is_current(epoch) {
            if outcome.is_ok() {
                self.compensate_superseded_success(epoch, enabled).await;
            }
            return Err(UpnpError::new(
                UpnpFailure::Superseded,
                "UPnP request superseded by a newer one",
            ));
        }

        match outcome {
            Ok(()) => {
                self.mapping_open.store(enabled, Ordering::SeqCst);
                Ok(())
            }
            Err(e) => Err(e),
        }
    }

    /// Один router-вызов на blocking pool с ограниченным ожиданием.
    async fn run_router_operation(
        self: &Arc<Self>,
        epoch: u64,
        enabled: bool,
    ) -> Result<(), UpnpError> {
        let budget = self.budget(enabled);
        let mut task = self.spawn_router_task(enabled);

        match tokio::time::timeout(budget, &mut task).await {
            Ok(Ok(result)) => result,
            Ok(Err(join_error)) => Err(UpnpError::new(
                UpnpFailure::TaskFailed,
                format!("UPnP router task failed: {}", join_error),
            )),
            Err(_) => {
                // Ожидание истекло, но blocking-задача продолжает выполняться:
                // её поздний успех не должен оставить нежелательный mapping.
                let manager = Arc::clone(self);
                tokio::spawn(async move {
                    manager.finish_late_operation(epoch, enabled, task).await;
                });
                Err(UpnpError::new(
                    UpnpFailure::Timeout,
                    format!(
                        "UPnP {} did not complete within {:?}",
                        if enabled {
                            "port forwarding"
                        } else {
                            "port unmapping"
                        },
                        budget
                    ),
                ))
            }
        }
    }

    fn spawn_router_task(&self, enabled: bool) -> tokio::task::JoinHandle<Result<(), UpnpError>> {
        let mapper = Arc::clone(&self.mapper);
        let port = self.port;
        tokio::task::spawn_blocking(move || {
            if enabled {
                mapper.open(port)
            } else {
                mapper.close(port)
            }
        })
    }

    /// Поздний результат операции, ожидание которой истекло: подтверждённый
    /// mapping не должен пережить stop/disable/restart.
    async fn finish_late_operation(
        self: Arc<Self>,
        epoch: u64,
        enabled: bool,
        task: tokio::task::JoinHandle<Result<(), UpnpError>>,
    ) {
        match task.await {
            Ok(Ok(())) => {
                if self.is_current(epoch) {
                    // Намерение не изменилось: поздний успех — это желаемое
                    // состояние, mapping можно подтвердить.
                    self.mapping_open
                        .store(self.desired.load(Ordering::SeqCst), Ordering::SeqCst);
                    tracing::info!(
                        port = self.port,
                        enabled,
                        "UPnP router operation completed after the wait"
                    );
                } else {
                    tracing::warn!(
                        port = self.port,
                        enabled,
                        "Late UPnP result belongs to a superseded request"
                    );
                    self.compensate_superseded_success(epoch, enabled).await;
                }
            }
            Ok(Err(e)) => tracing::warn!(
                error = %e,
                port = self.port,
                "Late UPnP router operation failed"
            ),
            Err(join_error) => tracing::warn!(
                error = %join_error,
                port = self.port,
                "Late UPnP router task failed"
            ),
        }
    }

    /// Устаревший успешный результат: если он противоречит текущему намерению,
    /// router физически приводится к этому намерению.
    async fn compensate_superseded_success(self: &Arc<Self>, epoch: u64, enabled: bool) {
        let desired = self.desired.load(Ordering::SeqCst);
        if enabled == desired {
            // Поздний результат совпадает с желаемым состоянием.
            if self.is_current(epoch) {
                self.mapping_open.store(desired, Ordering::SeqCst);
            }
            return;
        }

        tracing::warn!(
            port = self.port,
            enabled,
            desired,
            "UPnP result contradicts the current intent, compensating"
        );
        self.reconcile(desired).await;
    }

    /// Физически приводит router к состоянию `desired`. Используется только для
    /// компенсации: подтверждённый `mapping_open` здесь не доказательство, потому
    /// что отменённая операция могла успеть изменить router.
    async fn reconcile(self: &Arc<Self>, desired: bool) {
        let epoch = self.epoch.load(Ordering::SeqCst);
        let mut task = self.spawn_router_task(desired);

        match tokio::time::timeout(self.budget(desired), &mut task).await {
            Ok(Ok(Ok(()))) => {
                if self.is_current(epoch) {
                    self.mapping_open.store(desired, Ordering::SeqCst);
                }
                tracing::info!(port = self.port, desired, "UPnP state reconciled");
            }
            Ok(Ok(Err(e))) => tracing::warn!(
                error = %e,
                port = self.port,
                desired,
                "Failed to reconcile UPnP state"
            ),
            Ok(Err(join_error)) => tracing::warn!(
                error = %join_error,
                port = self.port,
                "UPnP reconcile task failed"
            ),
            Err(_) => tracing::warn!(
                port = self.port,
                desired,
                "UPnP reconcile did not complete within its budget"
            ),
        }
    }
}

impl Drop for UpnpManager {
    fn drop(&mut self) {
        if !self.mapping_open.load(Ordering::SeqCst) {
            return;
        }

        // Никогда не блокируем поток, уничтожающий менеджер: закрытие уходит в
        // blocking pool текущего runtime. Если runtime уже остановлен, mapping
        // снимется сам по истечении lease.
        let Ok(handle) = tokio::runtime::Handle::try_current() else {
            tracing::debug!(
                port = self.port,
                "No Tokio runtime available to remove UPnP mapping; lease will expire"
            );
            return;
        };

        let mapper = Arc::clone(&self.mapper);
        let port = self.port;
        handle.spawn_blocking(move || {
            if let Err(e) = mapper.close(port) {
                tracing::warn!(error = %e, port, "Failed to remove UPnP port mapping on drop");
            }
        });
    }
}

#[cfg(test)]
pub(crate) mod test_support {
    //! Scriptable router adapter: фиксирует вызовы, умеет блокировать открытие
    //! до разрешения теста и сообщать о начале операции.

    use super::{RouterPortMapper, UpnpError};
    use std::sync::mpsc::{channel, Receiver, Sender};
    use std::sync::Mutex;
    use tokio::sync::mpsc::{unbounded_channel, UnboundedReceiver, UnboundedSender};

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub(crate) enum RouterCall {
        Open(u16),
        Close(u16),
    }

    pub(crate) struct FakeRouterPortMapper {
        calls: Mutex<Vec<RouterCall>>,
        started: Mutex<Option<UnboundedSender<RouterCall>>>,
        open_gate: Mutex<Option<Receiver<()>>>,
    }

    impl FakeRouterPortMapper {
        pub(crate) fn new() -> Self {
            Self {
                calls: Mutex::new(Vec::new()),
                started: Mutex::new(None),
                open_gate: Mutex::new(None),
            }
        }

        /// Открытие блокируется до `release()`; о начале операции сообщается в
        /// возвращённый receiver. Закрытие выполняется сразу, поэтому тест может
        /// снять намерение, пока открытие ещё в полёте.
        pub(crate) fn gated_open() -> (Self, UnboundedReceiver<RouterCall>, Sender<()>) {
            let (started_tx, started_rx) = unbounded_channel();
            let (gate_tx, gate_rx) = channel();
            let mapper = Self {
                calls: Mutex::new(Vec::new()),
                started: Mutex::new(Some(started_tx)),
                open_gate: Mutex::new(Some(gate_rx)),
            };
            (mapper, started_rx, gate_tx)
        }

        pub(crate) fn calls(&self) -> Vec<RouterCall> {
            self.calls.lock().unwrap().clone()
        }

        pub(crate) fn open_count(&self) -> usize {
            self.calls()
                .iter()
                .filter(|call| matches!(call, RouterCall::Open(_)))
                .count()
        }

        pub(crate) fn close_count(&self) -> usize {
            self.calls()
                .iter()
                .filter(|call| matches!(call, RouterCall::Close(_)))
                .count()
        }

        fn record(&self, call: RouterCall) {
            self.calls.lock().unwrap().push(call);
            if let Some(started) = self.started.lock().unwrap().as_ref() {
                let _ = started.send(call);
            }
            if matches!(call, RouterCall::Open(_)) {
                if let Some(gate) = self.open_gate.lock().unwrap().as_ref() {
                    let _ = gate.recv();
                }
            }
        }
    }

    impl RouterPortMapper for FakeRouterPortMapper {
        fn open(&self, port: u16) -> Result<(), UpnpError> {
            self.record(RouterCall::Open(port));
            Ok(())
        }

        fn close(&self, port: u16) -> Result<(), UpnpError> {
            self.record(RouterCall::Close(port));
            Ok(())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::test_support::{FakeRouterPortMapper, RouterCall};
    use super::*;
    use netdev::prelude::{Ipv4Net, NetworkDevice};
    use std::collections::HashMap;
    use std::time::Instant;

    /// `IFF_UP` — `0x1` на всех платформах, которые поддерживает `netdev`.
    /// Тесты поднимают интерфейс явно, не завися от реальных адаптеров.
    const IFF_UP: u32 = 0x1;

    fn test_interface(name: &str, addrs: &[&str], gateway: Option<&str>, up: bool) -> Interface {
        let mut interface = Interface::dummy();
        interface.name = name.to_string();
        interface.flags = if up { IFF_UP } else { 0 };
        interface.ipv4 = addrs
            .iter()
            .map(|addr| Ipv4Net::new(addr.parse().unwrap(), 24).unwrap())
            .collect();
        interface.gateway = gateway.map(|gateway| {
            let mut device = NetworkDevice::new();
            device.ipv4 = vec![gateway.parse().unwrap()];
            device
        });
        interface
    }

    fn candidate(name: &str, addr: &str, gateway: Option<&str>) -> DiscoveryCandidate {
        DiscoveryCandidate {
            name: name.to_string(),
            addr: addr.parse().unwrap(),
            gateway: gateway.map(|gateway| gateway.parse().unwrap()),
        }
    }

    fn fake_gateway(ip: &str) -> Gateway {
        Gateway {
            addr: SocketAddrV4::new(ip.parse().unwrap(), 1900),
            root_url: "/rootDesc.xml".to_string(),
            control_url: "/control".to_string(),
            control_schema_url: "/schema".to_string(),
            control_schema: HashMap::new(),
        }
    }

    /// Тестовые бюджеты: достаточно короткие, чтобы timeout-путь был быстрым, и
    /// достаточно длинные, чтобы успешная операция не считалась timeout'ом.
    const TEST_BUDGET: Duration = Duration::from_millis(150);
    const LONG_BUDGET: Duration = Duration::from_secs(30);

    fn manager_with(
        mapper: Arc<dyn RouterPortMapper>,
        forward_timeout: Duration,
        remove_timeout: Duration,
    ) -> Arc<UpnpManager> {
        Arc::new(UpnpManager::with_mapper_and_timeouts(
            10100,
            mapper,
            forward_timeout,
            remove_timeout,
        ))
    }

    fn as_mapper(mapper: &Arc<FakeRouterPortMapper>) -> Arc<dyn RouterPortMapper> {
        Arc::clone(mapper) as Arc<dyn RouterPortMapper>
    }

    async fn wait_for_closes(mapper: &FakeRouterPortMapper, expected: usize) {
        let deadline = Instant::now() + Duration::from_secs(2);
        while mapper.close_count() < expected && Instant::now() < deadline {
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    }

    #[test]
    fn test_upnp_manager_creation() {
        let manager = UpnpManager::new(10100);
        assert_eq!(manager.port, 10100);
        assert!(!manager.is_desired());
        assert!(!manager.is_mapping_open());
    }

    #[tokio::test]
    async fn enabling_and_disabling_reaches_the_router_in_order() {
        let mapper = Arc::new(FakeRouterPortMapper::new());
        let manager = manager_with(as_mapper(&mapper), TEST_BUDGET, TEST_BUDGET);

        manager.set_enabled(true).await.unwrap();
        assert!(manager.is_mapping_open());
        assert!(manager.is_desired());

        manager.set_enabled(false).await.unwrap();
        assert!(!manager.is_mapping_open());
        assert_eq!(
            mapper.calls(),
            vec![RouterCall::Open(10100), RouterCall::Close(10100)]
        );
    }

    #[tokio::test]
    async fn restart_sequence_closes_before_reopening() {
        let mapper = Arc::new(FakeRouterPortMapper::new());
        let manager = manager_with(as_mapper(&mapper), TEST_BUDGET, TEST_BUDGET);

        manager.set_enabled(true).await.unwrap();
        manager.set_enabled(false).await.unwrap();
        manager.set_enabled(true).await.unwrap();

        assert_eq!(
            mapper.calls(),
            vec![
                RouterCall::Open(10100),
                RouterCall::Close(10100),
                RouterCall::Open(10100)
            ]
        );
        assert!(manager.is_mapping_open());
    }

    #[tokio::test]
    async fn slow_router_does_not_block_the_async_caller() {
        let (mapper, mut started, release) = FakeRouterPortMapper::gated_open();
        let mapper = Arc::new(mapper);
        let manager = manager_with(as_mapper(&mapper), LONG_BUDGET, TEST_BUDGET);

        let forward = {
            let manager = Arc::clone(&manager);
            tokio::spawn(async move { manager.set_enabled(true).await })
        };

        // Операция уже ушла в router, но ожидание не блокирует runtime: другие
        // задачи продолжают исполняться, пока router не ответил.
        let call = tokio::time::timeout(Duration::from_secs(2), started.recv())
            .await
            .expect("router operation must start")
            .expect("started channel must stay open");
        assert_eq!(call, RouterCall::Open(10100));
        assert!(!forward.is_finished());
        assert!(!manager.is_mapping_open());

        release.send(()).unwrap();
        forward.await.unwrap().unwrap();
        assert!(manager.is_mapping_open());
    }

    #[tokio::test]
    async fn late_mapping_after_disable_is_compensated() {
        let (mapper, mut started, release) = FakeRouterPortMapper::gated_open();
        let mapper = Arc::new(mapper);
        let manager = manager_with(as_mapper(&mapper), TEST_BUDGET, TEST_BUDGET);

        // Открытие не укладывается в бюджет: подтверждения нет.
        let timed_out = manager.set_enabled(true).await;
        assert!(timed_out.is_err(), "forward must report the timeout");
        assert_eq!(
            tokio::time::timeout(Duration::from_secs(2), started.recv())
                .await
                .expect("router operation must start")
                .expect("started channel must stay open"),
            RouterCall::Open(10100)
        );
        assert!(!manager.is_mapping_open());

        // Stop/disable снимает намерение и закрывает mapping, пока открытие
        // всё ещё висит в router.
        manager.set_enabled(false).await.unwrap();
        assert!(!manager.is_mapping_open());

        // Поздний успех открытия противоречит текущему намерению и гасится.
        release.send(()).unwrap();
        wait_for_closes(&mapper, 2).await;

        assert_eq!(mapper.open_count(), 1);
        assert_eq!(
            mapper.close_count(),
            2,
            "late mapping must be compensated after disable: {:?}",
            mapper.calls()
        );
        assert!(!manager.is_mapping_open());
    }

    #[tokio::test]
    async fn superseded_operation_is_not_confirmed_and_is_compensated() {
        let (mapper, mut started, release) = FakeRouterPortMapper::gated_open();
        let mapper = Arc::new(mapper);
        let manager = manager_with(as_mapper(&mapper), LONG_BUDGET, TEST_BUDGET);

        let forward = {
            let manager = Arc::clone(&manager);
            tokio::spawn(async move { manager.set_enabled(true).await })
        };
        assert_eq!(
            tokio::time::timeout(Duration::from_secs(2), started.recv())
                .await
                .expect("router operation must start")
                .expect("started channel must stay open"),
            RouterCall::Open(10100)
        );

        // Более новый запрос перехватывает владение, пока открытие в полёте.
        manager.set_enabled(false).await.unwrap();
        release.send(()).unwrap();

        let outcome = forward.await.unwrap();
        assert!(outcome.is_err(), "superseded forward must not be confirmed");
        assert!(!manager.is_mapping_open());

        wait_for_closes(&mapper, 2).await;
        assert_eq!(
            mapper.close_count(),
            2,
            "superseded open must be compensated: {:?}",
            mapper.calls()
        );
    }

    #[tokio::test]
    async fn late_compensation_cannot_close_a_newer_mapping_with_serialized_router_io() {
        use std::sync::atomic::AtomicUsize;
        use std::sync::mpsc::{channel, Receiver};
        use std::sync::Mutex;
        use tokio::sync::mpsc::{unbounded_channel, UnboundedSender};

        // Like IgdPortMapper: preparation of open can run before the gateway
        // lock, but the actual add_port/remove_port calls share one lock.
        struct SerializedMapper {
            mapping: Mutex<bool>,
            opens: AtomicUsize,
            closes: AtomicUsize,
            first_open_gate: Mutex<Receiver<()>>,
            second_close_gate: Mutex<Receiver<()>>,
            open_started: UnboundedSender<usize>,
            close_started: UnboundedSender<usize>,
        }

        impl RouterPortMapper for SerializedMapper {
            fn open(&self, _port: u16) -> Result<(), UpnpError> {
                let index = self.opens.fetch_add(1, Ordering::SeqCst);
                let _ = self.open_started.send(index);
                if index == 0 {
                    let _ = self.first_open_gate.lock().unwrap().recv();
                }
                *self.mapping.lock().unwrap() = true;
                Ok(())
            }

            fn close(&self, _port: u16) -> Result<(), UpnpError> {
                let index = self.closes.fetch_add(1, Ordering::SeqCst);
                let mut mapping = self.mapping.lock().unwrap();
                let _ = self.close_started.send(index);
                if index == 1 {
                    let _ = self.second_close_gate.lock().unwrap().recv();
                }
                *mapping = false;
                Ok(())
            }
        }

        let (release_first_open, first_open_gate) = channel();
        let (release_second_close, second_close_gate) = channel();
        let (open_started_tx, mut open_started_rx) = unbounded_channel();
        let (close_started_tx, mut close_started_rx) = unbounded_channel();
        let mapper = Arc::new(SerializedMapper {
            mapping: Mutex::new(false),
            opens: AtomicUsize::new(0),
            closes: AtomicUsize::new(0),
            first_open_gate: Mutex::new(first_open_gate),
            second_close_gate: Mutex::new(second_close_gate),
            open_started: open_started_tx,
            close_started: close_started_tx,
        });
        let manager = manager_with(mapper.clone(), LONG_BUDGET, LONG_BUDGET);

        let first = {
            let manager = Arc::clone(&manager);
            tokio::spawn(async move { manager.set_enabled(true).await })
        };
        assert_eq!(
            tokio::time::timeout(Duration::from_secs(2), open_started_rx.recv())
                .await
                .unwrap(),
            Some(0)
        );
        manager.set_enabled(false).await.unwrap();
        assert_eq!(
            tokio::time::timeout(Duration::from_secs(2), close_started_rx.recv())
                .await
                .unwrap(),
            Some(0)
        );

        release_first_open.send(()).unwrap();
        assert_eq!(
            tokio::time::timeout(Duration::from_secs(2), close_started_rx.recv())
                .await
                .unwrap(),
            Some(1)
        );

        let newer = {
            let manager = Arc::clone(&manager);
            tokio::spawn(async move { manager.set_enabled(true).await })
        };
        assert_eq!(
            tokio::time::timeout(Duration::from_secs(2), open_started_rx.recv())
                .await
                .unwrap(),
            Some(1)
        );
        let blocked_by_close = !newer.is_finished();
        release_second_close.send(()).unwrap();

        assert!(first.await.unwrap().is_err(), "old request was superseded");
        newer.await.unwrap().unwrap();
        assert!(blocked_by_close, "new open must wait for the old close");
        assert!(manager.is_desired());
        assert!(manager.is_mapping_open());
        assert!(
            *mapper.mapping.lock().unwrap(),
            "router mapping must remain open"
        );
    }

    #[tokio::test]
    async fn failure_is_reported_and_keeps_the_confirmed_state() {
        struct FailingMapper;

        impl RouterPortMapper for FailingMapper {
            fn open(&self, _port: u16) -> Result<(), UpnpError> {
                Err(UpnpError::new(
                    UpnpFailure::RouterRejected,
                    "router refused",
                ))
            }

            fn close(&self, _port: u16) -> Result<(), UpnpError> {
                Ok(())
            }
        }

        let manager = manager_with(Arc::new(FailingMapper), TEST_BUDGET, TEST_BUDGET);
        let error = manager.set_enabled(true).await.unwrap_err();
        assert_eq!(error.failure, UpnpFailure::RouterRejected);
        assert!(error.detail.contains("router refused"));
        assert_eq!(error.failure.code(), "webview.upnp.router_rejected");
        assert!(!manager.is_mapping_open());
    }

    #[tokio::test]
    async fn timeout_is_reported_as_its_own_failure_code() {
        let (mapper, mut started, _release) = FakeRouterPortMapper::gated_open();
        let mapper = Arc::new(mapper);
        let manager = manager_with(as_mapper(&mapper), TEST_BUDGET, TEST_BUDGET);

        let error = manager.set_enabled(true).await.unwrap_err();
        assert_eq!(error.failure, UpnpFailure::Timeout);
        assert_eq!(error.failure.code(), "webview.upnp.timeout");

        // Открытие так и осталось висеть в router: тест не должен его ждать.
        let _ = tokio::time::timeout(Duration::from_secs(1), started.recv()).await;
    }

    #[test]
    fn candidates_prefer_gateway_interfaces_and_filter_unusable_addresses() {
        let interfaces = vec![
            test_interface("virtual", &["192.168.56.1"], None, true),
            test_interface("lan", &["192.168.8.177"], Some("192.168.8.5"), true),
            test_interface("down", &["10.0.0.5"], Some("10.0.0.1"), false),
            test_interface(
                "weird",
                &[
                    "127.0.0.1",
                    "0.0.0.0",
                    "224.0.0.1",
                    "169.254.10.10",
                    "255.255.255.255",
                ],
                None,
                true,
            ),
        ];

        let candidates = eligible_candidates(&interfaces);

        assert_eq!(
            candidates
                .iter()
                .map(|candidate| candidate.addr)
                .collect::<Vec<_>>(),
            vec![
                "192.168.8.177".parse::<Ipv4Addr>().unwrap(),
                "192.168.56.1".parse::<Ipv4Addr>().unwrap(),
            ]
        );
        assert_eq!(candidates[0].gateway, Some("192.168.8.5".parse().unwrap()));
        assert_eq!(candidates[1].gateway, None);
    }

    #[test]
    fn candidates_deduplicate_shared_addresses_keeping_the_gateway_variant() {
        let interfaces = vec![
            test_interface("with-gateway", &["10.0.0.9"], Some("10.0.0.1"), true),
            test_interface("without-gateway", &["10.0.0.9"], None, true),
        ];

        let candidates = eligible_candidates(&interfaces);

        assert_eq!(candidates.len(), 1);
        assert_eq!(candidates[0].name, "with-gateway");
        assert_eq!(candidates[0].gateway, Some("10.0.0.1".parse().unwrap()));
    }

    #[test]
    fn candidates_deduplicate_non_adjacent_addresses_keeping_the_preferred_variant() {
        // После gateway-first сортировки повторы 10.0.0.9 отделены друг от друга
        // кандидатом 10.0.0.5: сначала идёт gateway-вариант, затем обычный.
        let interfaces = vec![
            test_interface("with-gateway", &["10.0.0.9"], Some("10.0.0.1"), true),
            test_interface("other", &["10.0.0.5"], None, true),
            test_interface("without-gateway", &["10.0.0.9"], None, true),
        ];

        let candidates = eligible_candidates(&interfaces);

        assert_eq!(
            candidates
                .iter()
                .map(|candidate| candidate.addr)
                .collect::<Vec<_>>(),
            vec![
                "10.0.0.9".parse::<Ipv4Addr>().unwrap(),
                "10.0.0.5".parse::<Ipv4Addr>().unwrap(),
            ]
        );
        let unique = candidates
            .iter()
            .map(|candidate| candidate.addr)
            .collect::<std::collections::HashSet<_>>();
        assert_eq!(unique.len(), candidates.len(), "addresses must be unique");
        assert_eq!(candidates[0].name, "with-gateway");
        assert_eq!(candidates[0].gateway, Some("10.0.0.1".parse().unwrap()));
        assert_eq!(candidates[1].gateway, None);
    }

    #[test]
    fn candidate_wait_is_clamped_by_the_shared_discovery_budget() {
        let budget = Duration::from_secs(10);
        let per_candidate = Duration::from_millis(3500);

        assert_eq!(
            next_candidate_wait(Duration::ZERO, budget, per_candidate),
            Some(per_candidate)
        );
        assert_eq!(
            next_candidate_wait(Duration::from_secs(9), budget, per_candidate),
            Some(Duration::from_secs(1))
        );
        assert_eq!(
            next_candidate_wait(Duration::from_secs(10), budget, per_candidate),
            None
        );
        assert_eq!(
            next_candidate_wait(Duration::from_secs(11), budget, per_candidate),
            None
        );
    }

    #[test]
    fn discovery_falls_back_to_the_next_candidate_after_a_failure() {
        let candidates = vec![
            candidate("first", "192.168.1.2", None),
            candidate("second", "192.168.1.3", None),
        ];
        let mut tried = Vec::new();
        let mut waits = Vec::new();

        let (gateway, selected) = discover_over_candidates(
            &candidates,
            Duration::from_secs(10),
            Duration::from_millis(3500),
            |candidate, wait| {
                tried.push(candidate.addr);
                waits.push(wait);
                if candidate.addr == "192.168.1.2".parse::<Ipv4Addr>().unwrap() {
                    Err("no response".to_string())
                } else {
                    Ok(fake_gateway("192.168.1.1"))
                }
            },
        )
        .unwrap();

        assert_eq!(
            tried,
            vec![
                "192.168.1.2".parse::<Ipv4Addr>().unwrap(),
                "192.168.1.3".parse::<Ipv4Addr>().unwrap(),
            ]
        );
        assert_eq!(waits, vec![Duration::from_millis(3500); 2]);
        assert_eq!(selected.name, "second");
        assert_eq!(
            *gateway.addr.ip(),
            "192.168.1.1".parse::<Ipv4Addr>().unwrap()
        );
    }

    #[test]
    fn discovery_without_eligible_candidates_reports_gateway_unavailable() {
        let error = discover_over_candidates(
            &[],
            Duration::from_secs(10),
            Duration::from_millis(3500),
            |_, _| Ok(fake_gateway("192.168.1.1")),
        )
        .unwrap_err();

        assert_eq!(error.failure, UpnpFailure::GatewayUnavailable);
        assert!(error.detail.contains("no eligible IPv4 interfaces"));
    }

    #[test]
    fn discovery_failure_reports_each_tried_candidate() {
        let candidates = vec![
            candidate("first", "192.168.1.2", None),
            candidate("second", "192.168.1.3", Some("192.168.1.1")),
        ];

        let error = discover_over_candidates(
            &candidates,
            Duration::from_secs(10),
            Duration::from_millis(3500),
            |_, _| Err("timed out".to_string()),
        )
        .unwrap_err();

        assert_eq!(error.failure, UpnpFailure::GatewayUnavailable);
        assert!(error.detail.contains("192.168.1.2"));
        assert!(error.detail.contains("192.168.1.3"));
    }

    /// Реальное обнаружение gateway на машине разработчика. Тест помечен
    /// `ignore`: он обращается к сети и не должен попадать в обычный прогон.
    /// Mapping не добавляется и не удаляется — только discovery и выбор
    /// локального адреса. Выводит gateway и локальный IP для ручной проверки.
    #[test]
    #[ignore = "queries the real network for a UPnP gateway; read-only, never adds or removes mappings"]
    fn real_gateway_discovery() {
        let mapper = IgdPortMapper::new();
        mapper
            .discover_gateway()
            .expect("a UPnP gateway should be discoverable on a UPnP network");
        let gateway_ip = mapper
            .cached_gateway_ip()
            .expect("discovered gateway must be cached");
        let local_ip = mapper
            .get_local_ip(gateway_ip)
            .expect("local route to the discovered gateway must resolve");

        eprintln!(
            "real_gateway_discovery: gateway={} mapping_local_ip={}",
            gateway_ip, local_ip
        );
    }
}
