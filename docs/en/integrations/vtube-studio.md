**English | [Русский](../../ru/integrations/vtube-studio.md)**

# VTube Studio

TTSBard sends typing state (`1` while typing, `0` after it stops) through VTube
Studio's Plugin API. It sends neither phrase text nor model audio.

## Before connecting

1. Start desktop VTube Studio and load your model.
2. Enable Plugin API; its usual port is `8001`. Use the actual port if different.
3. In TTSBard's VTube Studio panel, set the address/port if needed.
4. Connect; edited address/port are saved before connecting. Save alone stores
   them without connecting.
5. Approve TTSBard's access request in VTube Studio. Revoke it later in VTS
   Plugin API settings if needed.

Default address: `127.0.0.1` for this PC. Enter an IP/hostname without `ws://`
or port; use the neighboring port field. Wait for Connected. Connection alone
does not animate the model: configure a typing action next.
Changing address/port ends the current connection. After saving, reconnect
explicitly or through startup. The saved token remains; a different VTS server
may request approval again.

## VTube Studio on another PC (LAN)

On the VTS computer:

1. Enable Plugin API and note the actual port. If `8001` is occupied, VTS uses
   the next free port (`8002`, etc.), displayed in its settings.
2. Allow inbound `VTube Studio.exe` in Windows Firewall. This is a common cause
   of failures and may need re-adding after updates.
3. Find its LAN IP (`ipconfig`, for example `192.168.1.50`) or hostname.

On the TTSBard computer, enter that address/port, save and connect. Approve on
the remote VTS computer. Test reachability with:

```powershell
Test-NetConnection 192.168.1.50 -Port 8001
```

If TcpTestSucceeded is False, check address/port, API enabled, VTS running,
firewall and network reachability. VTS can advertise through UDP discovery,
but TTSBard uses an explicit address without depending on broadcast subnets.

## Configuration requires a connection

Typing action method, INPUT name, hotkeys, item and save-action control are
editable only while Connected. Offline, saved configuration is shown read-only
with a connection hint. Saving requires the INPUT to exist on the live VTS
server; there is no queued/background deferred write.

## Action methods

- **VTS parameter**: a custom VTS input with values `0..1`.
- **Hotkeys**: separate VTS hotkeys at typing start/stop.
- **Scene item**: show the selected GIF or another supported scene item while
  typing, then hide it afterward.

Parameter mode uses ParameterCreationRequest/InjectParameterDataRequest, not
the VTS event API. TTSBard creates/updates an INPUT; you map it to a model OUTPUT.

## Configure a VTS parameter

### INPUT and OUTPUT differ

```text
TTSBard INPUT TTSBardTyping → Live2D OUTPUT ParamTyping
```

`TTSBardTyping` is a custom/tracking INPUT created by TTSBard. `ParamTyping`
is a Live2D model OUTPUT created by the model author. A Live2D parameter does
not register an INPUT with that name. Sending directly to ParamTyping can
therefore produce error 453 like any missing input.

### Create and map INPUT

1. Choose VTS parameter in TTSBard.
2. Keep the recommended `TTSBardTyping` name and save the action.
3. If the name changes, the custom INPUT is created immediately over the live
   connection. Creation failure does **not** save the new action; the old one remains.
4. Test the action for a `0 → 1 → 0` pulse.
5. In VTS model settings, open **VTS Parameter Setup**.
6. Add a mapping or open the ParamTyping row.
7. Select TTSBardTyping as INPUT; VTS should identify TTSBard as its creator.
8. Select ParamTyping as OUTPUT.
9. For a binary indicator, use `0..1` for both ranges. Adapt only OUTPUT for a
   model with a different rig range.
10. Set smoothing to zero or the lowest suitable value and save model settings.
11. Test again: ParamTyping should follow `0 → 1 → 0`.

One Live2D OUTPUT can belong to only one ordinary input mapping. If another
row already controls it, decide which source should own it first.

## Configure hotkeys

1. In VTS model settings create separate typing-start/stop hotkeys.
2. Assign expressions, animations or other model actions.
3. Connect TTSBard and choose Hotkeys.
4. Click **Refresh hotkeys**, select start/stop actions and save.
5. Test both transitions, considering each hotkey's cooldown and expression toggles.

## Configure a scene item

1. Load the item into the current VTS scene; set position, size and layer.
2. Connect and choose Scene item.
3. Refresh items, select a file and save.
4. If multiple instances of the file exist, retain one; TTSBard does not pick
   an arbitrary instance.
5. Test: the item appears for the configured timeout, then hides.

## Rename a VTS parameter

For an INPUT rename, for example TTSBardTyping → MyTyping, TTSBard first creates
the new input, then saves/applies the action only on success, and finally deletes
the old input using ParameterDeletionRequest. A creation failure preserves the
old action. If deletion fails, the new action still works and a partial-success
message names both the new and undeleted inputs. Only custom inputs created
by TTSBard are removed; system/other-plugin inputs are not targeted.
Switching parameter → hotkeys/item → parameter does not delete the INPUT,
preserving its mapping for a later return.

## Troubleshooting

### Error 453

The INPUT name is missing. TTSBard creates it when saving a changed name,
or on the first test/typing event. Check that the field names an INPUT rather
than the Live2D OUTPUT; plugin access has not been revoked; and TTSBard is current.
Revocation deletes that plugin identity's custom inputs: reconnect and save/test
to recreate them.

### INPUT changes but the model does not

Check TTSBardTyping → ParamTyping mapping, OUTPUT range and smoothing.
Expressions, animations or physics may overwrite the same Live2D parameter.

### Error 454

Another plugin controls the parameter in set mode. Stop that source or use
a separate custom INPUT/mapping.

### Old INPUT deletion errors (400–403)

`400`: invalid name; `401`: already missing; `402`: another plugin's input;
`403`: system parameter. These produce partial success while the new action
works, naming the undeleted input. TTSBard intentionally avoids system/foreign inputs.

## Official references

- [VTube Studio Plugin API](https://github.com/DenchiSoft/VTubeStudio)
- [INPUT/OUTPUT mapping](https://github.com/DenchiSoft/VTubeStudio/wiki/VTS-Model-Settings#vts-parameter-setup)
- [API error IDs](https://github.com/DenchiSoft/VTubeStudio/blob/master/Files/ErrorID.cs)
