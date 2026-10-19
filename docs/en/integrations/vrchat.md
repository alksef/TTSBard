**English | [Русский](../../ru/integrations/vrchat.md)**

# VRChat: Chatbox output

Sends completed phrases to the avatar's chatbox using OSC over UDP and shows
a typing indicator. Draft text is not sent to VRChat.

## Enable OSC in VRChat

1. Open the radial Action Menu (`R` on keyboard or hold the headset controller's menu button).
2. Navigate **Expressions → Options → OSC**.
3. Set the **OSC** switch to the **Enabled** position.

## Connection settings

- **Host**: default `127.0.0.1` when both applications run on this PC.
- **Port**: default `9000`, VRChat's standard incoming OSC UDP port.
- **Start with the app**: enables output at launch. Without it, output starts
  disabled even if enabled during the previous session.
- **Send-only**: UDP delivery has no acknowledgement; TTSBard does not open a
  local listening socket. Output enabled/disabled does not confirm receipt.
  An enabled-output icon appears in the title bar and disappears when stopped.

After enabling output, send text in the test-message block. A sent-over-UDP
notification confirms sending the packet, not display in VRChat; panel errors
appear as notifications too.

## Chatbox limits and formatting

- **144 characters**: TTSBard conservatively counts Unicode scalar values.
  Longer copies are truncated with `…` without breaking grapheme clusters.
  Speech and original text stay unchanged.
- VRChat displays up to **9 lines** simultaneously.
- `/chatbox/input` carries `[string, direct=true, notification=true]`, showing
  the message immediately without opening the keyboard.

## Typing indicator

Typing sends `/chatbox/typing [true]`, showing three dots above the avatar.
The editor's idle timeout or sending a phrase produces `[false]`. Disable
typing delivery using the editor switch shared with other integrations.

## Routing

[Phrase routes](../user/editor.md#phrase-route) determine delivery:

- **Everywhere** (`everywhere`): voice and all enabled destinations, including VRChat.
- **No Twitch** (`no_twitch`): voice, WebView and VRChat.
- **Voice only** (`voice_only`): no VRChat.
- **Twitch only** (`twitch_only`): no VRChat.

For [Incoming](../user/incoming.md#automatic-acceptance), Everywhere/No Twitch
include VRChat; Voice and Voice + Twitch exclude it.
