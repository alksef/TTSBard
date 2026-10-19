**English | [Русский](../../ru/user/virtual-audio-cable.md)**

# Virtual audio cable

A virtual cable sends TTSBard speech to Discord, OBS, VTube Studio, games or
other applications as microphone input. TTSBard does not install this driver.
This guide uses **VB-CABLE Virtual Audio Device** by VB-Audio Software, a
third-party driver rather than a Windows or TTSBard component.

## Audio flow

The device names may seem reversed:

```text
TTSBard -> CABLE Input (playback device)
                    |
                    v
Discord/OBS/game <- CABLE Output (recording device / microphone)
```

- **CABLE Input (VB-Audio)** is where TTSBard sends playback audio.
- **CABLE Output (VB-Audio)** is selected as the microphone in the receiver.
- Everything sent to CABLE Input appears on CABLE Output.

## TTSBard output format

Under **Audio → Devices**, **Output format → Default** uses each device's
standard format. Try **32-bit integer (i32)** for ringing/distortion.
The setting is shared by Speaker and Virtual microphone, persists after restart
and applies when a new stream opens. Stop current audio, switch format and
restart the test. Effects/DSP remain in f32; only device delivery changes.
If i32 is unsupported at the current rate/channel count, an error is shown:
return to Default. See [audio devices](audio-devices.md) for comparison steps.

## Install VB-CABLE

1. Download from [VB-Audio's official page](https://vb-audio.com/Cable/).
2. Extract the entire archive.
3. Run the installer matching your Windows version as administrator and install.
4. Restart Windows as required by the driver vendor.

Use the official page for the current installer; filenames and package versions
can change. Avoid unofficial driver downloads.

## Configure TTSBard

1. Start TTSBard after restarting Windows.
2. Open **Audio**.
3. Enable **Virtual microphone**.
4. Select **CABLE Input (VB-Audio Virtual Cable)** under **Device**. The exact
   name may differ with driver version.
5. Adjust the virtual microphone volume.
6. Use the playback test button beside the device list.

Speaker and Virtual microphone are independent. Enable both to hear speech
locally and send it to another application simultaneously. Disable Speaker if
you do not need local playback; the virtual microphone continues working.

## Configure the receiver

Select **CABLE Output (VB-Audio Virtual Cable)** as the microphone/recording
device in Discord, OBS, VTube Studio, your game or another receiver.

1. Open the receiver's microphone meter or test.
2. Speak a short phrase in TTSBard using **Voice only**.
3. Check that the CABLE Output meter reacts and you can hear the test.
4. Adjust the virtual microphone volume if too quiet or overloaded.

## If the device is missing

1. Confirm Windows restarted after driver installation.
2. Check Windows sound settings: CABLE Input under outputs, CABLE Output under inputs.
3. Refresh TTSBard's device list under Audio.
4. Restart TTSBard if needed.
5. Check Windows sound settings and confirm that the devices are enabled.

If the TTSBard test works but the receiver is silent, select **CABLE Output**,
not CABLE Input or the physical microphone. To avoid feedback, do not enable
Windows monitoring of CABLE Output or route the receiver's output into the same cable.
