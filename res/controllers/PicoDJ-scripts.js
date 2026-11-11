// PicoDJ — mapping for the home-made "Bàn DJ ver 2" controller
// (Raspberry Pi Pico RP2040 / Pico W, USB MIDI + BLE MIDI, MIDI channel 1)
//
// Same layout as the Rekordbox mapping "Bàn DJ ver 2.csv":
//   buttons  Note 60..75   knobs CC10..19   faders CC22/23   encoder CC30 (relative: 1 = right, 127 = left)

var PicoDJ = {};

PicoDJ.decks = ["[Channel1]", "[Channel2]"];
PicoDJ.quickFx = ["[QuickEffectRack1_[Channel1]]", "[QuickEffectRack1_[Channel2]]"];

PicoDJ.init = function(_id, _debugging) {
    // FX1 works on deck 1 and FX2 on deck 2 by default (both can be re-assigned in the skin).
    engine.setValue("[EffectRack1_EffectUnit1]", "group_[Channel1]_enable", 1);
    engine.setValue("[EffectRack1_EffectUnit2]", "group_[Channel2]_enable", 1);
    engine.setValue("[EffectRack1_EffectUnit1]", "enabled", 1);
    engine.setValue("[EffectRack1_EffectUnit2]", "enabled", 1);
};

PicoDJ.shutdown = function() {
    // Turn every LED off (for controllers with LED feedback, ver 3+).
    for (let note = 0x3C; note <= 0x4B; note++) {
        midi.sendShortMsg(0x90, note, 0x00);
    }
};

// PLAY / PAUSE: toggles on press, ignores release.
PicoDJ.play = function(_channel, _control, value, _status, group) {
    if (value > 0) {
        script.toggleControl(group, "play");
    }
};

// SYNC: toggles sync lock on press (like Rekordbox BEAT SYNC).
PicoDJ.sync = function(_channel, _control, value, _status, group) {
    if (value > 0) {
        script.toggleControl(group, "sync_enabled");
    }
};

// FX1-1 ON / FX2-1 ON: toggles the first effect slot of the unit.
PicoDJ.fxOn = function(_channel, _control, value, _status, group) {
    if (value > 0) {
        script.toggleControl(group, "enabled");
    }
};

// CFX SELECT: one button steps both decks' Color FX to the next preset (Filter, Echo, Space, Crush, Noise…).
PicoDJ.cfxSelect = function(_channel, _control, value, _status, _group) {
    if (value > 0) {
        PicoDJ.quickFx.forEach(function(g) {
            engine.setValue(g, "next_chain_preset", 1);
        });
    }
};

// Encoder: relative CC (1..63 = right/down, 65..127 = left/up) scrolls the library.
PicoDJ.browse = function(_channel, _control, value, _status, _group) {
    const steps = value < 64 ? value : value - 128;
    if (steps !== 0) {
        engine.setValue("[Library]", "MoveVertical", steps);
    }
};
