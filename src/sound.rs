//! The ULTRA sound set: a Rust port of `extras/ultra/gen_sounds.py`. Same recipes, same
//! seed, same call order, so the output matches the reference script exactly (see the
//! tests, which bake in samples taken from the Python output). Nothing is shipped as an
//! audio file; every sound is synthesised at runtime into `$XDG_CACHE_HOME/sparklebios/sounds`.

use std::collections::HashMap;
use std::io;
use std::path::Path;

const RATE: f64 = 44100.0;
const PI: f64 = std::f64::consts::PI;

/// A Mersenne Twister (MT19937) generator matching CPython's `random.Random`, so the hard
/// disk chatter and the hiss underneath the modem and power off sounds draw the exact
/// numbers `gen_sounds.py` does when seeded the same way.
struct Rng {
    state: [u32; 624],
    index: usize,
}

impl Rng {
    /// A generator seeded the way CPython seeds `random.seed(n)` for a small non-negative
    /// integer `n`: the seed becomes a one word key fed to `init_by_array`.
    fn new(seed: u32) -> Self {
        let mut rng = Rng {
            state: [0; 624],
            index: 624,
        };
        rng.init_by_array(&[seed]);
        rng
    }

    fn init_genrand(&mut self, s: u32) {
        self.state[0] = s;
        for i in 1..624 {
            self.state[i] = (1_812_433_253u32
                .wrapping_mul(self.state[i - 1] ^ (self.state[i - 1] >> 30)))
            .wrapping_add(i as u32);
        }
        self.index = 624;
    }

    fn init_by_array(&mut self, key: &[u32]) {
        self.init_genrand(19_650_218);
        let mut i = 1usize;
        let mut j = 0usize;
        let mut k = 624.max(key.len());
        while k > 0 {
            self.state[i] = (self.state[i]
                ^ ((self.state[i - 1] ^ (self.state[i - 1] >> 30)).wrapping_mul(1_664_525)))
            .wrapping_add(key[j])
            .wrapping_add(j as u32);
            i += 1;
            j += 1;
            if i >= 624 {
                self.state[0] = self.state[623];
                i = 1;
            }
            if j >= key.len() {
                j = 0;
            }
            k -= 1;
        }
        k = 623;
        while k > 0 {
            self.state[i] = (self.state[i]
                ^ ((self.state[i - 1] ^ (self.state[i - 1] >> 30)).wrapping_mul(1_566_083_941)))
            .wrapping_sub(i as u32);
            i += 1;
            if i >= 624 {
                self.state[0] = self.state[623];
                i = 1;
            }
            k -= 1;
        }
        self.state[0] = 0x8000_0000;
        self.index = 624;
    }

    fn next_u32(&mut self) -> u32 {
        const N: usize = 624;
        const M: usize = 397;
        const MATRIX_A: u32 = 0x9908_b0df;
        const UPPER_MASK: u32 = 0x8000_0000;
        const LOWER_MASK: u32 = 0x7fff_ffff;
        if self.index >= N {
            for kk in 0..N {
                let y = (self.state[kk] & UPPER_MASK) | (self.state[(kk + 1) % N] & LOWER_MASK);
                let mag = if y & 1 == 1 { MATRIX_A } else { 0 };
                self.state[kk] = self.state[(kk + M) % N] ^ (y >> 1) ^ mag;
            }
            self.index = 0;
        }
        let mut y = self.state[self.index];
        self.index += 1;
        y ^= y >> 11;
        y ^= (y << 7) & 0x9d2c_5680;
        y ^= (y << 15) & 0xefc6_0000;
        y ^= y >> 18;
        y
    }

    /// Matches CPython's `random.random()`: a 53 bit float in `[0, 1)`.
    fn random(&mut self) -> f64 {
        let a = self.next_u32() >> 5;
        let b = self.next_u32() >> 6;
        (a as f64 * 67_108_864.0 + b as f64) * (1.0 / 9_007_199_254_740_992.0)
    }

    /// Matches CPython's `random.uniform(a, b)`.
    fn uniform(&mut self, a: f64, b: f64) -> f64 {
        a + (b - a) * self.random()
    }

    /// Matches CPython's `getrandbits(k)` for `k <= 32`.
    fn getrandbits(&mut self, k: u32) -> u32 {
        self.next_u32() >> (32 - k)
    }

    /// Matches CPython's `_randbelow(n)`, the rejection sampler behind `random.choice`.
    fn randbelow(&mut self, n: usize) -> usize {
        if n == 0 {
            return 0;
        }
        let k = u32::BITS - (n as u32).leading_zeros();
        loop {
            let r = self.getrandbits(k) as usize;
            if r < n {
                return r;
            }
        }
    }
}

/// A bandlimited square wave: odd harmonics up to 7kHz, so it sounds like a PC speaker
/// tone rather than the aliased buzz a naive square wave produces through a small speaker.
fn square(freq: f64, ms: f64, vol: f64, duty: f64) -> Vec<f64> {
    let n = (RATE * ms / 1000.0) as usize;
    let mut harmonics: Vec<u32> = (1..60)
        .step_by(2)
        .filter(|&k| freq * (k as f64) < 7000.0)
        .collect();
    if harmonics.is_empty() {
        harmonics.push(1);
    }
    let mut out = Vec::with_capacity(n);
    for i in 0..n {
        let t = i as f64 / RATE;
        let mut v: f64 = harmonics
            .iter()
            .map(|&k| (2.0 * PI * freq * k as f64 * t).sin() / k as f64)
            .sum();
        v *= 4.0 / PI;
        if duty != 0.5 {
            v = (v * 1.4 - (0.5 - duty) * 2.0).clamp(-1.0, 1.0);
        }
        let env = attack_release_envelope(i, n, 0.006, 0.012);
        out.push(vol * 0.8 * v * env);
    }
    out
}

fn silence(ms: f64) -> Vec<f64> {
    vec![0.0; (RATE * ms / 1000.0) as usize]
}

/// Band limited noise: white noise through a one pole low pass, so it hisses rather than
/// crackles.
fn noise(rng: &mut Rng, ms: f64, vol: f64, hold: f64) -> Vec<f64> {
    let n = (RATE * ms / 1000.0) as usize;
    let mut out = Vec::with_capacity(n);
    let alpha = (0.06 * hold).min(0.95);
    let mut lp = 0.0;
    for i in 0..n {
        let v = rng.uniform(-1.0, 1.0);
        lp += alpha * (v - lp);
        let env = min3(
            1.0,
            i as f64 / (RATE * 0.004),
            (n - i) as f64 / (RATE * 0.012),
        );
        out.push(lp * vol * 1.8 * env);
    }
    out
}

fn sweep(f0: f64, f1: f64, ms: f64, vol: f64) -> Vec<f64> {
    let n = (RATE * ms / 1000.0) as usize;
    let mut out = Vec::with_capacity(n);
    let mut ph = 0.0;
    for i in 0..n {
        let f = f0 + (f1 - f0) * i as f64 / n as f64;
        ph += f / RATE;
        let mut v: f64 = [1u32, 3, 5, 7]
            .iter()
            .filter(|&&k| f * (k as f64) < 7000.0)
            .map(|&k| (2.0 * PI * ph * k as f64).sin() / k as f64)
            .sum();
        v *= 4.0 / PI;
        let env = attack_release_envelope(i, n, 0.006, 0.012);
        out.push(vol * 0.8 * v * env);
    }
    out
}

fn mix(a: &[f64], b: &[f64]) -> Vec<f64> {
    let n = a.len().max(b.len());
    (0..n)
        .map(|i| {
            let x = a.get(i).copied().unwrap_or(0.0);
            let y = b.get(i).copied().unwrap_or(0.0);
            (x + y).tanh()
        })
        .collect()
}

fn notes(seq: &[(f64, f64)], vol: f64, duty: f64) -> Vec<f64> {
    let mut out = Vec::new();
    for &(freq, ms) in seq {
        if freq != 0.0 {
            out.extend(square(freq, ms, vol, duty));
        } else {
            out.extend(silence(ms));
        }
    }
    out
}

/// The 5ms attack, 12ms (or, for noise, 4ms and 12ms) release envelope every tone and
/// noise burst shares.
fn attack_release_envelope(i: usize, n: usize, attack_secs: f64, release_secs: f64) -> f64 {
    min3(
        1.0,
        i as f64 / (RATE * attack_secs),
        (n - i) as f64 / (RATE * release_secs),
    )
}

fn min3(a: f64, b: f64, c: f64) -> f64 {
    a.min(b).min(c)
}

/// A one pole resonator driven by impulses: the drive's case ringing at `freq` Hz for a
/// few milliseconds after each head seek.
fn resonator(impulses: &HashMap<usize, f64>, freq: f64, decay_ms: f64, n: usize) -> Vec<f64> {
    let mut out = vec![0.0; n];
    let r = (-1.0 / (RATE * decay_ms / 1000.0)).exp();
    let c = 2.0 * r * (2.0 * PI * freq / RATE).cos();
    let mut y1 = 0.0;
    let mut y2 = 0.0;
    for (i, slot) in out.iter_mut().enumerate() {
        let impulse = impulses.get(&i).copied().unwrap_or(0.0);
        let y = impulse + c * y1 - r * r * y2;
        *slot = y;
        y2 = y1;
        y1 = y;
    }
    out
}

/// The irregular bursts of head seeks behind `hdd_chatter`: an impulse of a random
/// strength every few tens of milliseconds, in groups, with a longer gap between groups.
fn hdd_knocks(rng: &mut Rng, n_hdd: usize) -> HashMap<usize, f64> {
    let mut knocks = HashMap::new();
    let mut i = (RATE * 0.1) as usize;
    const BURST_SIZES: [usize; 5] = [1, 2, 4, 7, 12];
    while i < n_hdd - 2000 {
        let count = BURST_SIZES[rng.randbelow(BURST_SIZES.len())];
        for _ in 0..count {
            knocks.insert(i, rng.uniform(0.5, 1.0));
            i += (RATE * rng.uniform(0.012, 0.04)) as usize;
            if i >= n_hdd - 2000 {
                break;
            }
        }
        i += (RATE * rng.uniform(0.08, 0.5)) as usize;
    }
    knocks
}

/// The final one pole smoothing and fade out every sound gets before it is quantised,
/// matching `polish()` in the Python script.
fn polish(samples: &[f64]) -> Vec<f64> {
    let alpha = 0.55;
    let n = samples.len();
    let fade = (RATE * 0.015) as usize;
    let mut lp = 0.0;
    let mut out = Vec::with_capacity(n);
    for (i, &x) in samples.iter().enumerate() {
        lp += alpha * (x - lp);
        let remaining = n - i;
        let env = if remaining < fade {
            (remaining as f64 / fade as f64).min(1.0)
        } else {
            1.0
        };
        out.push((lp * env).clamp(-1.0, 1.0));
    }
    out
}

/// Polishes, then quantises to 16 bit signed PCM the way the Python script's `write()`
/// does: `int(sample * 32767)`, truncating toward zero.
fn quantize(samples: &[f64]) -> Vec<i16> {
    polish(samples)
        .iter()
        .map(|&s| (s * 32767.0) as i16)
        .collect()
}

const C4: f64 = 261.6;
const D4: f64 = 293.7;
const E4: f64 = 329.6;
const G4: f64 = 392.0;
const A4: f64 = 440.0;
const B4: f64 = 493.9;
const C5: f64 = 523.3;
const D5: f64 = 587.3;
const E5: f64 = 659.3;
const G5: f64 = 784.0;
const A5: f64 = 880.0;
const C6: f64 = 1046.5;

/// A flavour name, its note sequence (frequency in Hz, duration in ms; a zero frequency
/// is a rest), and the duty cycle its notes are played with.
type Jingle = (&'static str, Vec<(f64, f64)>, f64);

/// One jingle per built in flavour, six notes or fewer, in roster order (see
/// `flavour::builtins`): unicorn, sumo, ninja, viking, luchador, yeti, raccoon, wizard.
fn jingles() -> Vec<Jingle> {
    vec![
        (
            "unicorn",
            vec![
                (C5, 90.0),
                (E5, 90.0),
                (G5, 90.0),
                (C6, 90.0),
                (G5, 90.0),
                (C6, 260.0),
            ],
            0.5,
        ),
        (
            "sumo",
            vec![
                (98.0, 220.0),
                (0.0, 120.0),
                (98.0, 220.0),
                (0.0, 120.0),
                (73.0, 420.0),
            ],
            0.5,
        ),
        ("ninja", vec![(A5, 40.0), (0.0, 260.0), (A4, 40.0)], 0.25),
        ("viking", vec![(D4, 260.0), (A4, 260.0), (D5, 520.0)], 0.5),
        (
            "luchador",
            vec![
                (E5, 110.0),
                (E5, 110.0),
                (G5, 110.0),
                (E5, 110.0),
                (B4, 110.0),
                (E5, 330.0),
            ],
            0.5,
        ),
        (
            "yeti",
            vec![(C4, 380.0), (B4 / 2.0, 380.0), (A4 / 2.0, 600.0)],
            0.5,
        ),
        (
            "raccoon",
            vec![
                (G4, 70.0),
                (0.0, 50.0),
                (G4, 70.0),
                (0.0, 50.0),
                (E4, 70.0),
                (0.0, 140.0),
                (C5, 120.0),
            ],
            0.5,
        ),
        (
            "wizard",
            vec![
                (A4, 80.0),
                (C5, 80.0),
                (E5, 80.0),
                (A5, 80.0),
                (C6, 80.0),
                (E5 * 2.0, 380.0),
            ],
            0.25,
        ),
    ]
}

/// The fifteen ULTRA sounds, in the order `gen_sounds.py` writes them, as raw (unpolished,
/// unquantised) samples. A single `Rng` seeded at 4 is threaded through in that order,
/// because `hdd_chatter`'s seek knocks and the hiss under `modem` and `power_off` draw
/// from the same random stream in the reference script (its `random.seed(1985)` at the
/// top is dead: nothing draws from it before the seed is reset to 4 for the knocks).
fn recipes() -> Vec<(String, Vec<f64>)> {
    let mut rng = Rng::new(4);
    let mut out = Vec::with_capacity(15);

    out.push(("post_ok".to_string(), square(1000.0, 140.0, 0.35, 0.5)));
    out.push((
        "post_fail".to_string(),
        [
            square(880.0, 160.0, 0.35, 0.5),
            silence(240.0),
            square(880.0, 160.0, 0.35, 0.5),
            silence(240.0),
            square(880.0, 160.0, 0.35, 0.5),
        ]
        .concat(),
    ));

    let mut seek = Vec::new();
    for &f in &[92.0, 92.0, 104.0, 92.0, 116.0, 104.0] {
        seek.extend(square(f, 38.0, 0.3, 0.2));
        seek.extend(silence(14.0));
    }
    out.push(("floppy_seek".to_string(), seek));

    let n_hdd = (RATE * 6.0) as usize;
    let knocks = hdd_knocks(&mut rng, n_hdd);
    let seeks = resonator(&knocks, 1100.0, 4.0, n_hdd);
    let peak = seeks.iter().fold(0.0_f64, |m, &v| m.max(v.abs()));
    let peak = if peak == 0.0 { 1.0 } else { peak };
    out.push((
        "hdd_chatter".to_string(),
        seeks.iter().map(|&v| 0.45 * v / peak).collect(),
    ));
    out.push(("hdd_spindown".to_string(), sweep(240.0, 40.0, 900.0, 0.12)));

    let mut modem = Vec::new();
    for &f in &[941.0, 1209.0, 697.0, 1336.0, 852.0] {
        modem.extend(mix(
            &square(f, 60.0, 0.18, 0.5),
            &square(f * 1.31, 60.0, 0.18, 0.5),
        ));
        modem.extend(silence(35.0));
    }
    modem.extend(silence(120.0));
    modem.extend(square(2100.0, 380.0, 0.22, 0.5));
    modem.extend(silence(60.0));
    for i in 0..14 {
        modem.extend(square(
            if i % 2 != 0 { 1200.0 } else { 2250.0 },
            45.0,
            0.2,
            0.5,
        ));
    }
    modem.extend(mix(
        &noise(&mut rng, 700.0, 0.22, 2.0),
        &sweep(1800.0, 900.0, 700.0, 0.1),
    ));
    out.push(("modem".to_string(), modem));

    out.push((
        "power_off".to_string(),
        mix(
            &noise(&mut rng, 90.0, 0.3, 6.0),
            &sweep(15000.0 / 8.0, 60.0, 520.0, 0.12),
        ),
    ));

    for (name, seq, duty) in jingles() {
        out.push((format!("jingle_{name}"), notes(&seq, 0.26, duty)));
    }

    out
}

const HEADER_LEN: usize = 44;

/// Builds a 16 bit mono 44.1kHz WAV file's bytes by hand: the RIFF/WAVE header followed
/// by little endian signed 16 bit PCM data.
fn wav_bytes(pcm: &[i16]) -> Vec<u8> {
    let data_len = pcm.len() * 2;
    let mut bytes = Vec::with_capacity(HEADER_LEN + data_len);
    bytes.extend_from_slice(b"RIFF");
    bytes.extend_from_slice(&((36 + data_len) as u32).to_le_bytes());
    bytes.extend_from_slice(b"WAVE");
    bytes.extend_from_slice(b"fmt ");
    bytes.extend_from_slice(&16u32.to_le_bytes()); // fmt chunk size, PCM
    bytes.extend_from_slice(&1u16.to_le_bytes()); // audio format, PCM
    bytes.extend_from_slice(&1u16.to_le_bytes()); // channels, mono
    let rate = RATE as u32;
    bytes.extend_from_slice(&rate.to_le_bytes()); // sample rate
    bytes.extend_from_slice(&(rate * 2).to_le_bytes()); // byte rate
    bytes.extend_from_slice(&2u16.to_le_bytes()); // block align
    bytes.extend_from_slice(&16u16.to_le_bytes()); // bits per sample
    bytes.extend_from_slice(b"data");
    bytes.extend_from_slice(&(data_len as u32).to_le_bytes());
    for sample in pcm {
        bytes.extend_from_slice(&sample.to_le_bytes());
    }
    bytes
}

/// The fifteen sound names, in the order `recipes()` produces them.
const SOUND_NAMES: [&str; 15] = [
    "post_ok",
    "post_fail",
    "floppy_seek",
    "hdd_chatter",
    "hdd_spindown",
    "modem",
    "power_off",
    "jingle_unicorn",
    "jingle_sumo",
    "jingle_ninja",
    "jingle_viking",
    "jingle_luchador",
    "jingle_yeti",
    "jingle_raccoon",
    "jingle_wizard",
];

/// Synthesises every ULTRA sound that is missing from `dir` and writes it as a 16 bit
/// mono 44.1kHz WAV file, leaving whatever is already there untouched. Creates `dir` if
/// needed. When every sound already exists this is just fifteen `stat` calls; otherwise
/// it is cheap enough (well under 200ms for all fifteen) to call on every `bios init` and
/// every `bios sprinkles ultra`.
pub fn ensure(dir: &Path) -> io::Result<()> {
    let already_have_everything = SOUND_NAMES
        .iter()
        .all(|name| dir.join(format!("{name}.wav")).exists());
    if already_have_everything {
        return Ok(());
    }
    std::fs::create_dir_all(dir)?;
    for (name, raw) in recipes() {
        let path = dir.join(format!("{name}.wav"));
        if path.exists() {
            continue;
        }
        std::fs::write(path, wav_bytes(&quantize(&raw)))?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sound(name: &str) -> Vec<i16> {
        let recipes = recipes();
        let (_, raw) = recipes.iter().find(|(n, _)| n.as_str() == name).unwrap();
        quantize(raw)
    }

    fn peak(samples: &[i16]) -> f64 {
        samples
            .iter()
            .map(|&s| (s as f64).abs())
            .fold(0.0, f64::max)
            / 32768.0
    }

    // Samples pulled from a run of extras/ultra/gen_sounds.py (seed 1985, then reseeded to
    // 4 for the knocks, exactly as the script does) at fixed offsets into each sound. If
    // this test fails, the Rust port has drifted from the Python reference: fix the port,
    // do not touch these numbers.
    const CASES: &[(&str, &[(usize, i16)])] = &[
        (
            "post_ok",
            &[
                (0, 0),
                (6, 182),
                (61, 2103),
                (308, -5830),
                (617, -4706),
                (1543, -5091),
                (3086, -6853),
                (4629, -8293),
                (5555, -7995),
                (6111, -96),
            ],
        ),
        (
            "post_fail",
            &[
                (0, 0),
                (42, -1478),
                (423, 10298),
                (2116, 9281),
                (4233, 9117),
                (10583, 0),
                (21167, 8522),
                (31751, 0),
                (38101, 9232),
                (41911, 5007),
            ],
        ),
        (
            "floppy_seek",
            &[
                (0, 0),
                (13, 313),
                (137, 3198),
                (687, 6103),
                (1375, -4470),
                (3437, 6295),
                (6875, 0),
                (10313, 6674),
                (12375, 6367),
                (13613, 0),
            ],
        ),
        (
            "hdd_chatter",
            &[
                (0, 0),
                (264, 0),
                (2645, 0),
                (13229, -317),
                (26459, 0),
                (66149, 0),
                (132299, 0),
                (198449, 0),
                (238139, 1),
                (261953, 0),
            ],
        ),
        (
            "hdd_spindown",
            &[
                (0, 0),
                (39, 463),
                (396, 2996),
                (1984, -3612),
                (3968, -3375),
                (9922, 2816),
                (19844, 144),
                (29766, 2808),
                (35720, -93),
                (39292, -1283),
            ],
        ),
        (
            "modem",
            &[
                (0, 0),
                (104, -130),
                (1042, 9171),
                (5214, 317),
                (10428, 9435),
                (26071, 0),
                (52143, -4762),
                (78214, 4509),
                (93857, 6741),
                (103243, 4078),
            ],
        ),
        (
            "power_off",
            &[
                (0, 0),
                (22, -666),
                (229, -445),
                (1146, -1337),
                (2293, 1425),
                (5732, 2803),
                (11465, -1592),
                (17198, -3350),
                (20637, 3023),
                (22701, -447),
            ],
        ),
        (
            "jingle_unicorn",
            &[
                (0, 0),
                (31, 751),
                (313, -6640),
                (1565, -6419),
                (3131, 6593),
                (7827, -1464),
                (15655, -3157),
                (23482, 6725),
                (28179, -7282),
                (30996, -2054),
            ],
        ),
        (
            "jingle_sumo",
            &[
                (0, 0),
                (48, 1222),
                (485, 6908),
                (2425, 6823),
                (4850, -6858),
                (12127, 0),
                (24254, -5780),
                (36381, -6728),
                (43658, -6868),
                (48023, -4605),
            ],
        ),
        (
            "jingle_ninja",
            &[
                (0, 0),
                (14, 293),
                (149, -3816),
                (749, -6815),
                (1499, -3423),
                (3748, 0),
                (7496, 0),
                (11244, 0),
                (13493, -6752),
                (14843, 423),
            ],
        ),
        (
            "jingle_viking",
            &[
                (0, 0),
                (45, 1123),
                (458, 6498),
                (2293, 6923),
                (4586, -6408),
                (11465, 23),
                (22931, 22),
                (34397, -6646),
                (41276, 7054),
                (45404, 4060),
            ],
        ),
        (
            "jingle_luchador",
            &[
                (0, 0),
                (38, -1012),
                (388, -6536),
                (1940, -1679),
                (3880, -932),
                (9701, 9),
                (19403, 9),
                (29105, 556),
                (34926, -4648),
                (38418, -3014),
            ],
        ),
        (
            "jingle_yeti",
            &[
                (0, 0),
                (59, 1469),
                (599, -6878),
                (2998, -6859),
                (5997, -6694),
                (14993, -6963),
                (29987, 6674),
                (44981, 6767),
                (53977, 6811),
                (59375, -1077),
            ],
        ),
        (
            "jingle_raccoon",
            &[
                (0, 0),
                (25, 609),
                (251, 6316),
                (1256, 6685),
                (2513, 6634),
                (6284, -6913),
                (12568, -6734),
                (18852, 0),
                (22622, -7026),
                (24884, -1215),
            ],
        ),
        (
            "jingle_wizard",
            &[
                (0, 0),
                (34, 793),
                (343, 6268),
                (1719, 6280),
                (3439, 1001),
                (8599, 6294),
                (17198, 5297),
                (25797, -6815),
                (30957, 5351),
                (34053, -2321),
            ],
        ),
    ];

    #[test]
    fn matches_the_python_reference_sample_for_sample() {
        for (name, cases) in CASES {
            let samples = sound(name);
            for &(offset, expected) in *cases {
                assert_eq!(
                    samples[offset], expected,
                    "{name} sample {offset} did not match the Python reference"
                );
            }
        }
    }

    #[test]
    fn every_sound_stays_under_half_scale() {
        for (name, raw) in recipes() {
            let p = peak(&quantize(&raw));
            assert!(p < 0.5, "{name} peak {p} is not under 0.5");
        }
    }

    #[test]
    fn fifteen_sounds_in_the_expected_order() {
        let names: Vec<_> = recipes().into_iter().map(|(n, _)| n.to_string()).collect();
        assert_eq!(
            names,
            vec![
                "post_ok",
                "post_fail",
                "floppy_seek",
                "hdd_chatter",
                "hdd_spindown",
                "modem",
                "power_off",
                "jingle_unicorn",
                "jingle_sumo",
                "jingle_ninja",
                "jingle_viking",
                "jingle_luchador",
                "jingle_yeti",
                "jingle_raccoon",
                "jingle_wizard",
            ]
        );
    }

    fn read_wav_header(bytes: &[u8]) -> (u16, u32, u16, u32) {
        // Returns (channels, sample_rate, bits_per_sample, data_len).
        assert_eq!(&bytes[0..4], b"RIFF");
        assert_eq!(&bytes[8..12], b"WAVE");
        assert_eq!(&bytes[12..16], b"fmt ");
        let channels = u16::from_le_bytes([bytes[22], bytes[23]]);
        let sample_rate = u32::from_le_bytes([bytes[24], bytes[25], bytes[26], bytes[27]]);
        let bits_per_sample = u16::from_le_bytes([bytes[34], bytes[35]]);
        assert_eq!(&bytes[36..40], b"data");
        let data_len = u32::from_le_bytes([bytes[40], bytes[41], bytes[42], bytes[43]]);
        (channels, sample_rate, bits_per_sample, data_len)
    }

    #[test]
    fn ensure_writes_fifteen_valid_wav_files_then_nothing_on_a_second_call() {
        let dir = tempfile::tempdir().unwrap();
        ensure(dir.path()).unwrap();
        let mut entries: Vec<_> = std::fs::read_dir(dir.path())
            .unwrap()
            .map(|e| e.unwrap().path())
            .collect();
        entries.sort();
        assert_eq!(entries.len(), 15);
        for entry in &entries {
            let bytes = std::fs::read(entry).unwrap();
            let (channels, sample_rate, bits_per_sample, data_len) = read_wav_header(&bytes);
            assert_eq!(channels, 1);
            assert_eq!(sample_rate, 44100);
            assert_eq!(bits_per_sample, 16);
            assert_eq!(bytes.len(), HEADER_LEN + data_len as usize);
        }

        let mtimes_before: Vec<_> = entries
            .iter()
            .map(|p| std::fs::metadata(p).unwrap().modified().unwrap())
            .collect();
        let start = std::time::Instant::now();
        ensure(dir.path()).unwrap();
        assert!(start.elapsed() < std::time::Duration::from_millis(50));
        let mtimes_after: Vec<_> = entries
            .iter()
            .map(|p| std::fs::metadata(p).unwrap().modified().unwrap())
            .collect();
        assert_eq!(mtimes_before, mtimes_after);
    }

    #[test]
    fn generating_all_fifteen_is_within_budget() {
        let dir = tempfile::tempdir().unwrap();
        let start = std::time::Instant::now();
        ensure(dir.path()).unwrap();
        let elapsed = start.elapsed();
        // The 200ms budget is what `bios` promises in the real, optimised binary. A debug
        // build skips the optimiser entirely and cargo test's default parallelism adds
        // contention on top, so debug gets a much looser regression guard instead of the
        // real number; that real number is measured against a release build and reported
        // separately, not asserted here.
        let budget = if cfg!(debug_assertions) {
            std::time::Duration::from_millis(2000)
        } else {
            std::time::Duration::from_millis(200)
        };
        assert!(elapsed < budget, "took {elapsed:?}, budget {budget:?}");
    }
}
