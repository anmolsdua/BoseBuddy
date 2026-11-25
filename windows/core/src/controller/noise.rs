//! Noise modes: switch, edit (ANC / Wind Block), create Custom.

use super::*;

impl Controller {
    pub fn set_mode(&mut self, index: u8) {
        if !self.state.link.is_connected() {
            return;
        }
        self.begin("mode");
        let previous = self.state.mode_index;
        self.state.mode_index = Some(index);
        // Second byte asks the headset to play the mode prompt, like the
        // physical Action button.
        let result = self
            .exchange(31, 3, Op::Start, &[index, 1], true, false, Duration::from_secs(3))
            .and_then(|packets| ensure_ok(&packets, 31, 3));
        match result {
            Ok(()) => {
                let name = self.mode_name(index);
                self.log.write(format!("mode -> {name}"));
                self.pause(Duration::from_millis(250));
                self.read_mode_index();
            }
            Err(error) => {
                self.state.mode_index = previous;
                self.set_error(error.to_string());
            }
        }
        self.end();
    }

    /// Rewrite an editable mode's noise cancelling. The headset keeps the
    /// change, so it survives reconnects and the Action button cycle.
    pub fn update_mode(&mut self, index: u8, raw_cnc: u8, wind_block: bool) {
        let Some(mode) = self.state.modes.iter().find(|m| m.index == index).cloned() else { return };
        if !self.state.link.is_connected() || !mode.editable {
            return;
        }
        self.begin("mode-edit");
        if let Some(m) = self.state.modes.iter_mut().find(|m| m.index == index) {
            m.raw_cnc = if wind_block { 0 } else { raw_cnc.min(10) };
            m.wind_block = wind_block;
        }
        let payload = mode.config_payload(raw_cnc, wind_block);
        match self
            .exchange(31, 6, Op::SetGet, &payload, true, false, DEFAULT_TIMEOUT)
            .and_then(|packets| ensure_ok(&packets, 31, 6))
        {
            Ok(()) => self.log.write(format!("mode {index} set cnc={raw_cnc} wind={wind_block}")),
            Err(error) => {
                self.log.write(format!("mode {index} write failed: {error}"));
                self.set_error(error.to_string());
            }
        }
        self.load_modes();
        self.end();
    }

    /// Create a mode named "Custom" in the free slot, starting from the
    /// current noise cancelling level, and switch to it.
    pub fn create_custom_mode(&mut self) {
        if !self.state.link.is_connected() || self.state.busy.is_some() {
            return;
        }
        let Some(slot) = self.state.free_mode_slot else { return };
        self.begin("mode-edit");
        let current = self.state.current_mode().cloned();
        let mut raw_cnc = current.as_ref().map_or(0, |m| m.raw_cnc);
        if current.as_ref().is_some_and(|m| m.wind_block) {
            // Wind Block modes store 0; use the level it is applying now.
            if let Ok(live) = self.get(1, 5, &[]) {
                if let Some(&level) = live.payload.get(1) {
                    raw_cnc = level;
                }
            }
        }
        let mode = BoseMode::custom(slot, raw_cnc);
        let result = self
            .exchange(31, 6, Op::SetGet, &mode.config_payload(raw_cnc, false), true, false, DEFAULT_TIMEOUT)
            .and_then(|packets| ensure_ok(&packets, 31, 6));
        if let Err(error) = result {
            self.log.write(format!("create Custom mode failed: {error}"));
            self.set_error(error.to_string());
            self.end();
            return;
        }
        self.log.write(format!("created Custom mode in slot {slot} cnc={raw_cnc}"));
        // Show it right away instead of waiting for the next GetAll.
        self.state.free_mode_slot = None;
        self.state.modes.retain(|m| m.index != slot);
        self.state.modes.push(mode);
        self.state.modes.sort_by_key(|m| m.index);
        self.end();
        self.set_mode(slot);
        self.load_modes();
    }

    pub(super) fn mode_name(&self, index: u8) -> String {
        self.state.modes.iter().find(|m| m.index == index).map(|m| m.name.clone()).unwrap_or_else(|| index.to_string())
    }
}
