**English | [Русский](../../ru/integrations/twitch.md)**

# Twitch Chat

Duplicate TTS phrases in Twitch Chat using either connection mode.

## 1. Twitch API mode

Uses Twitch Helix `POST /helix/chat/messages` to send as an authorized bot
account to a streamer's chat. The bot does **not** need channel moderator rights.

1. Create an application in the [Twitch Developer Console](https://dev.twitch.tv/console/apps),
   category Chat Bot, client type Confidential.
2. Enter Client ID/Client Secret and save.
3. Authorize the bot: explicitly open the activation link and approve using
   a separate bot account (`user:write:chat`, `user:bot`).
4. Add a channel and approve using its owner's account (`channel:bot`). Copy
   the link to the owner or open it in another browser/device. The browser does
   not open automatically. Keep TTSBard open; waiting is limited to five minutes.
5. Select one recipient channel. The first added channel is selected automatically.
6. Connect to activate delivery.

Both accounts use Device Code Flow, without a local callback. If registration
requires an OAuth Redirect URL, `http://localhost` can remain; code-based login
does not redirect there.
Resetting the bot clears its local login, cancels pending authorization and
stops API delivery, retaining Client ID/Secret. Revoke access on Twitch itself
under Twitch settings → Connections. Changing recipients while connected
restarts delivery for the newly selected channel.

Removing channel access forgets that grant **locally**, deleting saved tokens
on this PC without a Twitch revoke request. Add it again if needed. Removing
the selected channel clears the recipient and stops API delivery.

## 2. Classic IRC mode

Connect directly through an IRC socket:

1. Obtain a bot token using [twitchtokengenerator.com](https://twitchtokengenerator.com).
2. Copy only the OAuth token, without `oauth:`.
3. Enter bot username, target channel and token, then connect.

- **Username**: the token's account, 1–25 Latin letters, digits or `_`, no `@`/spaces.
- **Token**: no `oauth:` prefix.
- **Channel**: name only, no URL/`#`, for example `twitchdev`; same 1–25-character rules.

An initial IRC connection unconfirmed for 30 seconds fails. Check channel and
network, then retry manually. Temporary network loss or Twitch `RECONNECT`
triggers automatic reconnection, shown in the title bar. Manual disconnect
stops retries. A phrase interrupted during delivery is not resent automatically;
resend only if appropriate.

Select an [editor route](../user/editor.md#phrase-route) including Twitch.
HTTP/OCR use the separate [Incoming route](../user/incoming.md#automatic-acceptance),
which defaults to voice only.

Field-format errors remain until corrected. A username/token-account mismatch
stops connecting with a specific error. Connection errors remain until retry or
disconnect. Connect/reconnect saves edited credentials before connecting;
the operation notification reports saving separately.
Logs include failure codes/retry details, not raw library IRC traffic even at
TRACE: PASS/PRIVMSG can contain tokens and user text.

## Panel layout and credentials

Connection controls and two shared checkboxes are above mode/credentials/save.
API bot and channel panels are stacked below. There is no Redirect URL field;
both accounts sign in by code. A separate authorization reset retains application
credentials. Client ID is hidden until requested. A stored Client Secret is
shown as a saved status: the pencil opens an empty replacement field, the cross
cancels and Save replaces it only after a successful write. The eye reveals only
new input. Changing mode stops the connection until manually started, including
when the new mode's credentials are incomplete.
Several authorized channels may be stored, with one selected recipient.
Authorization links open/copy explicitly; removal is local, not a Twitch revoke.
