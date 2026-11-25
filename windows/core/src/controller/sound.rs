//! EQ bands and presets, sidetone, multipoint.

use super::*;

impl Controller {
    pub fn set_eq(&mut self, band_id: u8, value: i32) {
        let Some(band) = self.state.eq_bands.iter().find(|b| b.band_id == band_id).cloned() else { return };
        if !self.state.link.is_connected() {
            return;
        }
        let clamped = band.clamp(value);
        self.begin(&format!("eq:{band_id}"));
        if let Some(b) = self.state.eq_bands.iter_mut().find(|b| b.band_id == band_id) {
            b.value = clamped;
        }
        match self.write_eq(band_id, clamped) {
            Ok(()) => self.log.write(format!("EQ {} -> {clamped}", band.name)),
            Err(error) => self.set_error(error.to_string()),
        }
        self.load_eq();
        self.end();
    }

    /// Apply a whole preset. Only bands that differ are written.
    pub fn apply_eq(&mut self, values: &[i32]) {
        if !self.state.link.is_connected() || self.state.eq_bands.is_empty() {
            return;
        }
        self.begin("eq:preset");
        let bands = self.state.eq_bands.clone();
        let mut result = Ok(());
        for (i, (band, value)) in bands.iter().zip(values).enumerate() {
            let clamped = band.clamp(*value);
            if clamped == band.value {
                continue;
            }
            self.state.eq_bands[i].value = clamped;
            result = self.write_eq(band.band_id, clamped);
            if result.is_err() {
                break;
            }
        }
        match result {
            Ok(()) => self.log.write(format!(
                "EQ preset -> {}",
                values.iter().map(i32::to_string).collect::<Vec<_>>().join("/")
            )),
            Err(error) => self.set_error(error.to_string()),
        }
        self.load_eq();
        self.end();
    }

    pub(super) fn write_eq(&mut self, band_id: u8, value: i32) -> Result<(), BmapError> {
        let byte = value.clamp(i8::MIN as i32, i8::MAX as i32) as i8 as u8;
        let packets = self.exchange(1, 7, Op::SetGet, &[byte, band_id], true, false, DEFAULT_TIMEOUT)?;
        ensure_ok(&packets, 1, 7)
    }

    pub fn set_sidetone(&mut self, level: u8) {
        if !self.state.link.is_connected() {
            return;
        }
        self.begin("sidetone");
        let previous = self.state.sidetone;
        self.state.sidetone = level;
        match self
            .exchange(1, 11, Op::SetGet, &[1, level], true, false, DEFAULT_TIMEOUT)
            .and_then(|packets| ensure_ok(&packets, 1, 11))
        {
            Ok(()) => {
                self.log.write(format!("sidetone -> {level}"));
                self.load_sidetone();
            }
            Err(error) => {
                self.state.sidetone = previous;
                self.set_error(error.to_string());
            }
        }
        self.end();
    }

    pub fn set_multipoint(&mut self, enabled: bool) {
        if !self.state.link.is_connected() {
            return;
        }
        self.begin("multipoint");
        let previous = self.state.multipoint;
        self.state.multipoint = enabled;
        match self
            .exchange(1, 10, Op::SetGet, &[enabled as u8], true, false, DEFAULT_TIMEOUT)
            .and_then(|packets| ensure_ok(&packets, 1, 10))
        {
            Ok(()) => {
                self.log.write(format!("multipoint -> {}", if enabled { "on" } else { "off" }));
                self.pause(Duration::from_millis(200));
                self.load_multipoint();
                self.load_peers();
            }
            Err(error) => {
                self.state.multipoint = previous;
                self.set_error(error.to_string());
            }
        }
        self.end();
    }
}
