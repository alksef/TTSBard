# selfbuild_tts.py для LunaTranslator -> TTSBard (app-tts-v2)
#
# Сервер: раздел «Входящий сервер» приложения TTSBard
#   POST http://127.0.0.1:10101/v1/speech   Content-Type: application/json
#   Тело: {"text": "реплика"} (лимит тела 64 КиБ)
#   Ответ 202: {"status": "queued", "job_id": ...} при включённом
#               автовоспроизведении, либо
#              {"status": "pending_review", "incoming_id": ...} если текст
#               ждёт подтверждения во «Входящих».
#   Ошибки — JSON {"code", "message", "retryable"}:
#     429 очередь или «Входящие» переполнены, 422 пустой/слишком длинный текст,
#     503 провайдер не готов, 409 прочее, 400/413/415 разбор запроса.
#   Синтез и воспроизведение выполняет TTSBard через активный провайдер
#   (препроцессор, ударения, эффекты, очередь, аудиовыходы). Аудио наружу
#   не возвращается — этот скрипт только передаёт текст.
#
# Установка: скопировать в <папка LunaTranslator>/userconfig/selfbuild_tts.py
# (или через UI: настройки TTS -> движок «自定义» -> карандаш, вставить, сохранить).
# Движок «自定义» ищите во вкладке ОНЛАЙН (квирк классификации Luna).
#
# В TTSBard: раздел «Входящий сервер» -> «Запустить»; «Автовоспроизведение»
# включить, иначе реплики копятся во «Входящих» и ждут подтверждения.
# Подробнее: docs/integrations/lunatranslator/README.md в репозитории TTSBard.

import os

import requests

from tts.basettsclass import TTSbase, SpeechParam

URL = os.environ.get("TTSBARD_URL", "http://127.0.0.1:10101")


class TTS(TTSbase):

    # синтезом и воспроизведением владеет TTSBard — ползунки Luna не действуют
    arg_support_pitch = False
    arg_support_speed = False

    def init(self): ...

    def getvoicelist(self):
        # провайдер и голос выбираются в TTSBard, списка тут нет
        return None  # -> [""] / ["Default"]

    def speak(self, content, voice, param: SpeechParam):
        text = content.strip()
        if not text:
            return None
        try:
            response = self.proxysession.post(
                URL.rstrip("/") + "/v1/speech",
                json={"text": text},
                timeout=15,  # loopback; сервер отвечает сразу после приёма в очередь
            )
        except requests.RequestException as e:
            raise Exception(
                "TTSBard недоступен на {} — включите «Входящий сервер» в TTSBard ({})".format(
                    URL, e
                )
            )
        if response.status_code == 202:
            # принято (queued | pending_review): озвучит TTSBard, плеер Luna молчит.
            # None не попадает в LRU-кэш Luna, поэтому повторы фраз снова уходят
            # на сервер и озвучиваются каждый раз.
            return None
        # ошибка: {"code": "...", "message": "...", "retryable": ...}
        try:
            message = response.json().get("message", response.text)
        except Exception:
            message = response.text
        raise Exception(
            "TTSBard: HTTP {} — {}".format(response.status_code, message)
        )
