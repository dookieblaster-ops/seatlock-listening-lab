//! Room models (README section 5.10): `free`, which is no room, and `simple`, an estimate from
//! the wall behind the speaker and the room's diffuse sound.

use num_complex::Complex64;
use seatlock_core::constants::{C0, RHO0};
use seatlock_core::registry::{InRoom, RoomImage, RoomModel, RoomSound};
use serde_json::{Map, Value};

pub struct FreeRoom;

impl RoomModel for FreeRoom {
    fn at_listener(&self, direct: &[Complex64]) -> Vec<Complex64> {
        direct.to_vec()
    }
}

pub fn parse_free(_fields: &Map<String, Value>) -> Result<Box<dyn RoomModel>, String> {
    Ok(Box::new(FreeRoom))
}

/// Sabine's equivalent absorption area, `A = 0.161 * V / RT60`, in m^2.
pub fn absorption_area_m2(volume_m3: f64, rt60_s: f64) -> f64 {
    0.161 * volume_m3 / rt60_s
}

/// RMS pressure of the diffuse sound for a radiated power: `p^2 = 4 * W * RHO0*C0 / A`.
pub fn diffuse_pressure_pa(power_w: f64, absorption_m2: f64) -> f64 {
    (4.0 * power_w.max(0.0) * RHO0 * C0 / absorption_m2).sqrt()
}

/// One wall behind the speaker and an even reverberant field.
pub struct SimpleRoom {
    pub wall_behind_m: f64,
    pub wall_reflection: f64,
    pub volume_m3: f64,
    pub rt60_s: f64,
}

impl RoomModel for SimpleRoom {
    fn at_listener(&self, direct: &[Complex64]) -> Vec<Complex64> {
        direct.to_vec()
    }

    /// The cabinet mirrored in the wall behind it is the same as the listening position
    /// mirrored in that wall, heard from the real cabinet.
    fn images(&self, listener_m: [f64; 3], cabinet_size_m: [f64; 3]) -> Vec<RoomImage> {
        if self.wall_reflection == 0.0 {
            return Vec::new();
        }
        let wall_y = cabinet_size_m[1] + self.wall_behind_m;
        vec![RoomImage {
            point_m: [listener_m[0], 2.0 * wall_y - listener_m[1], listener_m[2]],
            gain: self.wall_reflection,
        }]
    }

    fn needs_sound_power(&self) -> bool {
        true
    }

    fn in_room(&self, sound: &RoomSound) -> Option<InRoom> {
        let bins = sound.freqs_hz.len();
        let mut reflection = vec![Complex64::new(0.0, 0.0); bins];
        for image in sound.images {
            for (slot, value) in reflection.iter_mut().zip(image) {
                *slot += *value * self.wall_reflection;
            }
        }
        let area = absorption_area_m2(self.volume_m3, self.rt60_s);
        let diffuse_pa = (0..bins)
            .map(|bin| diffuse_pressure_pa(sound.power_w.get(bin).copied().unwrap_or(0.0), area))
            .collect();
        Some(InRoom {
            reflection,
            diffuse_pa,
        })
    }

    fn notes(&self) -> Vec<String> {
        vec![
            "The room is an estimate: the wall behind the speaker and an even reverberant \
             sound. No other walls, no floor, and no room resonances."
                .into(),
            "The wall's reflection ignores the cabinet standing in its way back.".into(),
        ]
    }
}

pub fn parse_simple(fields: &Map<String, Value>) -> Result<Box<dyn RoomModel>, String> {
    let number = |key: &str| -> Result<f64, String> {
        fields
            .get(key)
            .and_then(Value::as_f64)
            .ok_or_else(|| format!("The room needs a number for {key}."))
    };
    let room = SimpleRoom {
        wall_behind_m: number("wall_behind_m")?,
        wall_reflection: number("wall_reflection")?,
        volume_m3: number("volume_m3")?,
        rt60_s: number("rt60_s")?,
    };
    if room.wall_behind_m < 0.0 {
        return Err(
            "The wall cannot be in front of the cabinet's back: wall_behind_m is negative.".into(),
        );
    }
    if !(0.0..=1.0).contains(&room.wall_reflection) {
        return Err("The wall's reflection is a fraction from 0 to 1.".into());
    }
    if room.volume_m3 <= 0.0 || room.rt60_s <= 0.0 {
        return Err("The room's volume and reverberation time must be above zero.".into());
    }
    Ok(Box::new(room))
}
