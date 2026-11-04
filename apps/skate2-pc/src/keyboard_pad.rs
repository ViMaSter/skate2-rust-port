//! PC port addition: the keyboard as controller one. W, A, S and D are the left stick; U, I, J and
//! K are X, Y, A and B, laid out as the face buttons are; the space bar holds the right stick down
//! and letting it go returns the stick to the middle, the ollie's wind-up and release. Keys are
//! read by position, so other keyboard layouts keep the same places. The keys join a pad in slot
//! 0; with none there, slot 0 reads as a wired gamepad once one of these keys is pressed.
use skate2_core::XboxPadSample;
use winit::keyboard::KeyCode;

/// `XINPUT_GAMEPAD_A`, `_B`, `_X` and `_Y`.
const BUTTON_A: u16 = 0x1000;
const BUTTON_B: u16 = 0x2000;
const BUTTON_X: u16 = 0x4000;
const BUTTON_Y: u16 = 0x8000;

/// A stick held along one axis.
const FULL: i16 = i16::MAX;
/// Each axis of a stick held on a diagonal: on the circle a pad's stick travels, not its corner.
const DIAGONAL: i16 = 23170;

/// The slot the keyboard plays in.
const SLOT: u32 = 0;

#[derive(Clone, Copy, PartialEq, Eq)]
enum Binding {
    Button(u16),
    /// A bit of `KeyboardPad::left`.
    Left(u8),
    RightStickDown,
}

const UP: u8 = 1;
const DOWN: u8 = 2;
const LEFT: u8 = 4;
const RIGHT: u8 = 8;

fn binding(key: KeyCode) -> Option<Binding> {
    Some(match key {
        KeyCode::KeyW => Binding::Left(UP),
        KeyCode::KeyS => Binding::Left(DOWN),
        KeyCode::KeyA => Binding::Left(LEFT),
        KeyCode::KeyD => Binding::Left(RIGHT),
        KeyCode::KeyU => Binding::Button(BUTTON_X),
        KeyCode::KeyI => Binding::Button(BUTTON_Y),
        KeyCode::KeyJ => Binding::Button(BUTTON_A),
        KeyCode::KeyK => Binding::Button(BUTTON_B),
        KeyCode::Space => Binding::RightStickDown,
        _ => return None,
    })
}

/// One axis from its two keys: both or neither held is the middle.
fn axis(negative: bool, positive: bool) -> i16 {
    i16::from(positive) - i16::from(negative)
}

#[derive(Default)]
pub(crate) struct KeyboardPad {
    buttons: u16,
    /// The held left-stick keys, `UP` to `RIGHT`.
    left: u8,
    right_stick_down: bool,
    /// A key has been pressed: slot 0 reads as connected even without a pad.
    used: bool,
    /// Counts changes, as `XInput`'s packet number does.
    changes: u32,
}

impl KeyboardPad {
    /// A key went down or up; true when it is one of the keyboard pad's.
    pub(crate) fn key(&mut self, key: KeyCode, pressed: bool) -> bool {
        let Some(binding) = binding(key) else {
            return false;
        };
        let before = (self.buttons, self.left, self.right_stick_down);
        match binding {
            Binding::Button(bit) if pressed => self.buttons |= bit,
            Binding::Button(bit) => self.buttons &= !bit,
            Binding::Left(bit) if pressed => self.left |= bit,
            Binding::Left(bit) => self.left &= !bit,
            Binding::RightStickDown => self.right_stick_down = pressed,
        }
        if before != (self.buttons, self.left, self.right_stick_down) {
            self.changes = self.changes.wrapping_add(1);
        }
        self.used |= pressed;
        true
    }

    /// Every key let go: a key released while the window had no focus sends no release.
    pub(crate) fn release_all(&mut self) {
        if self.buttons != 0 || self.left != 0 || self.right_stick_down {
            self.changes = self.changes.wrapping_add(1);
        }
        self.buttons = 0;
        self.left = 0;
        self.right_stick_down = false;
    }

    /// Whether a slot that found no pad reads as connected.
    pub(crate) fn connected(&self, index: u32) -> bool {
        index == SLOT && self.used
    }

    /// The left stick's x and y (up positive, as `XInput`'s), when a key holds it.
    fn left_stick(&self) -> Option<(i16, i16)> {
        let held = |bit| self.left & bit != 0;
        let (x, y) = (axis(held(LEFT), held(RIGHT)), axis(held(DOWN), held(UP)));
        let reach = if x != 0 && y != 0 { DIAGONAL } else { FULL };
        (x != 0 || y != 0).then(|| (x * reach, y * reach))
    }

    /// A slot's packet number and sample with the keys on it; a slot that found no pad, `None`,
    /// gets the keys alone once the keyboard has been used.
    pub(crate) fn over(&self, index: u32, pad: Option<(u32, XboxPadSample)>) -> Option<(u32, XboxPadSample)> {
        if index != SLOT {
            return pad;
        }
        let (packet, mut sample) = match pad {
            Some(pad) => pad,
            None if self.used => (0, XboxPadSample { buttons: 0, left_trigger: 0, right_trigger: 0, left_x: 0, left_y: 0, right_x: 0, right_y: 0 }),
            None => return None,
        };
        sample.buttons |= self.buttons;
        if let Some((x, y)) = self.left_stick() {
            (sample.left_x, sample.left_y) = (x, y);
        }
        if self.right_stick_down {
            sample.right_y = i16::MIN;
        }
        Some((packet.wrapping_add(self.changes), sample))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const STILL: XboxPadSample = XboxPadSample { buttons: 0, left_trigger: 0, right_trigger: 0, left_x: 0, left_y: 0, right_x: 0, right_y: 0 };

    #[test]
    fn keys_are_the_face_buttons_and_space_holds_the_right_stick_down() {
        let mut keys = KeyboardPad::default();
        assert_eq!(keys.over(0, None), None, "unused, an empty slot stays empty");
        assert!(!keys.connected(0));
        assert!(!keys.key(KeyCode::KeyE, true));
        for (key, bit) in [(KeyCode::KeyU, BUTTON_X), (KeyCode::KeyI, BUTTON_Y), (KeyCode::KeyJ, BUTTON_A), (KeyCode::KeyK, BUTTON_B)] {
            assert!(keys.key(key, true));
            assert_eq!(keys.over(0, None).unwrap().1.buttons, bit);
            keys.key(key, false);
        }
        assert!(keys.connected(0) && !keys.connected(1));
        keys.key(KeyCode::Space, true);
        assert_eq!(keys.over(0, None).unwrap().1, XboxPadSample { right_y: i16::MIN, ..STILL });
        keys.key(KeyCode::Space, false);
        assert_eq!(keys.over(0, None).unwrap().1, STILL, "released, the stick is back in the middle");
    }

    #[test]
    fn wasd_is_the_left_stick() {
        let mut keys = KeyboardPad::default();
        let left = |keys: &KeyboardPad| {
            let sample = keys.over(0, None).unwrap().1;
            (sample.left_x, sample.left_y)
        };
        keys.key(KeyCode::KeyW, true);
        assert_eq!(left(&keys), (0, FULL));
        keys.key(KeyCode::KeyD, true);
        assert_eq!(left(&keys), (DIAGONAL, DIAGONAL));
        keys.key(KeyCode::KeyS, true);
        assert_eq!(left(&keys), (FULL, 0), "up and down cancel");
        keys.key(KeyCode::KeyW, false);
        keys.key(KeyCode::KeyD, false);
        keys.key(KeyCode::KeyA, true);
        assert_eq!(left(&keys), (-DIAGONAL, -DIAGONAL));
        keys.release_all();
        assert_eq!(left(&keys), (0, 0));
    }

    #[test]
    fn keys_join_the_pad_in_slot_zero_only() {
        let mut keys = KeyboardPad::default();
        let pad = XboxPadSample { buttons: 0x10, left_x: 300, left_y: -400, right_x: 900, right_y: 1200, ..STILL };
        keys.key(KeyCode::KeyJ, true);
        keys.key(KeyCode::Space, true);
        let (packet, sample) = keys.over(0, Some((5, pad))).unwrap();
        assert_eq!(sample, XboxPadSample { buttons: 0x10 | BUTTON_A, right_y: i16::MIN, ..pad }, "the pad's left stick stays while no key holds it");
        assert_ne!(packet, 5, "a key's change is a new packet");
        keys.key(KeyCode::KeyA, true);
        assert_eq!(keys.over(0, Some((5, pad))).unwrap().1.left_x, -FULL);
        assert_eq!(keys.over(1, Some((5, pad))), Some((5, pad)));
        assert_eq!(keys.over(1, None), None);
        keys.release_all();
        assert_eq!(keys.over(0, Some((5, pad))).unwrap().1, pad);
    }
}
