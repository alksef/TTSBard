# DECISION-022 — Twitch IRC lifecycle принадлежит twitch-irc

**Статус:** `accepted` (ROADMAP-104)
**Связано:** [ROADMAP-104](../roadmap/active/104-twitch-chat-client-and-delivery-resilience.md)

> **TL;DR.** Транспорт и протокольный lifecycle Twitch IRC (TCP/TLS, PING/PONG,
> RECONNECT, повторный JOIN) отданы библиотеке `twitch-irc`. Приложение сохраняет
> владение runtime-настройками, UI-статусом и исходящей доставкой; supervisor
> реагирует backoff-ретраем только на смерть runtime. Запись в сокет — это
> локальная отправка (`sent`), не подтверждение отображения в чате.

## Контекст

Самописный TCP/TLS IRC (PASS/NICK/JOIN, PING/PONG, RECONNECT, reconnect backoff)
демонстрировал «молчаливую смерть» доставки; корректность зависела от ручной
синхронизации reader/writer/listener.

## Решение

Транспорт и протокольный lifecycle отданы `twitch-irc` 6.1.1 (MIT; компилируемая
зависимость, не bundled — в THIRD_PARTY_NOTICES не входит). Приложение сохраняет
владение runtime-настройками, UI-статусом, исходящей доставкой.

Разделение владельцев повторного соединения:

- connection-level reconnect и повторный JOIN — библиотека (встроены, не отключаются);
- supervisor (`servers/twitch.rs`) реагирует backoff-ретраем ТОЛЬКО на смерть
  runtime (`TransportFailure`: recv None без stop, DropGuard фоновой задачи).

Статусы: `Connected` только после подтверждённого JOIN (`ServerMessage::Join` с
нашим логином в целевом канале); NOTICE auth-failure — терминальный `Error` c
принудительной остановкой runtime (иначе библиотека реконнектится бесконечно);
тихий обрыв библиотека не эмитит — он обнаруживается опросом `get_channel_status`
(мёртвое соединение с подтверждёнными JOIN'ами удаляется немедленно, поэтому
`(wanted, joined) = (true, false)` держится всю фазу реконнекта и понижает
`Connected` до `Connecting` до подтверждённого повторного JOIN); запись в сокет
не считается подтверждённой доставкой — контракт команды `deliver_twitch_message`
возвращает `sent`, а не `delivered`.

## Исходящая доставка

- Очередь bounded (20) + pacer 1500 мс: 20 сообщений/30 с обычного аккаунта и
  не более 1/с; повышенные лимиты (broadcaster/mod/VIP) не предполагаются.
- Переполнение: typed `twitch.queue_full` (retryable), сообщение не теряется молча.
- Неоднозначная ошибка записи НЕ повторяет сообщение автоматически (анти-дубликат).
- Диагностика: длина очереди/interval/reconnect generation; текст сообщений и
  token в логи не пишутся.

## Последствия

- Stop/Restart детерминированы: drop последнего handle библиотеки завершает её
  фоновые задачи; supervisor не дублирует connection-reconnect.
- Миграция на EventSub/Helix Send Chat Message API остаётся отдельным решением
  (серверный is_sent/drop_reason), см. roadmap.
