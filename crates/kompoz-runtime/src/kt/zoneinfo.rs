//! The system's time zone database (what the JDK's `ZoneInfo` reads from its own `tzdb.dat`): the IANA zones of the
//! TZif files under `TZDIR`, `/usr/share/zoneinfo` (RFC 8536: their transitions, then the POSIX `TZ` rule of their
//! footer for the times after the last), Android's tzdata bundle of them, or a platform's own database (the web's);
//! and which zone the system is in (`TZ`, the platform's word, else the system's: `/etc/localtime`, Android's
//! `persist.sys.timezone`, Windows' zone).

use std::path::{Path, PathBuf};
use std::sync::Arc;

/// A zone's rules: its offsets from UTC over time.
#[derive(Debug)]
pub(crate) struct ZoneRules {
    /// Its transitions, in UTC seconds, each with the index of the local time type from then on.
    transitions: Vec<(i64, usize)>,
    types: Vec<LocalType>,
    /// The rule after the last transition (the footer).
    rule: Option<PosixRule>,
}

/// A local time type: its offset from UTC, whether it is daylight saving time, its abbreviation (`CET`, `+03`).
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct LocalType {
    pub(crate) offset: i32,
    pub(crate) dst: bool,
    pub(crate) abbreviation: String,
}

/// What a zone says of one instant: its offset (seconds), the standard part of it, its abbreviation.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct Offsets {
    pub(crate) total: i32,
    pub(crate) standard: i32,
    pub(crate) dst: bool,
    pub(crate) abbreviation: String,
}

impl ZoneRules {
    /// A zone always `offset` seconds from UTC.
    pub(crate) fn fixed(offset: i32, abbreviation: &str) -> ZoneRules {
        ZoneRules {
            transitions: Vec::new(),
            types: vec![LocalType { offset, dst: false, abbreviation: abbreviation.to_string() }],
            rule: None,
        }
    }

    /// The zone of a TZif file's bytes; none if they aren't one.
    pub(crate) fn from_tzif(data: &[u8]) -> Option<ZoneRules> {
        let header = Header::read(data)?;
        let (header, body, wide) = if header.version >= b'2' {
            // The version 2+ data, 64-bit times, after the version 1 block.
            let v1 = 44 + header.body_len(4);
            let second = Header::read(data.get(v1..)?)?;
            (second, data.get(v1 + 44..)?, true)
        } else {
            (header, data.get(44..)?, false)
        };
        let time_len = if wide { 8 } else { 4 };
        let mut at = 0;
        let mut take = |n: usize| {
            let s = body.get(at..at + n);
            at += n;
            s
        };
        let times = take(header.timecnt * time_len)?;
        let indices = take(header.timecnt)?;
        let infos = take(header.typecnt * 6)?;
        let chars = take(header.charcnt)?;
        take(header.leapcnt * (time_len + 4))?;
        take(header.isstdcnt)?;
        take(header.isutcnt)?;
        let footer_start = at;
        let types: Vec<LocalType> = infos
            .chunks(6)
            .map(|t| {
                let start = t[5] as usize;
                let end =
                    chars.get(start..).and_then(|c| c.iter().position(|&b| b == 0)).map_or(chars.len(), |n| n + start);
                LocalType {
                    offset: i32::from_be_bytes([t[0], t[1], t[2], t[3]]),
                    dst: t[4] != 0,
                    abbreviation: String::from_utf8_lossy(chars.get(start..end).unwrap_or_default()).into_owned(),
                }
            })
            .collect();
        if types.is_empty() {
            return None;
        }
        let transitions = (0..header.timecnt)
            .map(|i| {
                let t = &times[i * time_len..(i + 1) * time_len];
                let time = if wide {
                    i64::from_be_bytes(t.try_into().expect("8 bytes"))
                } else {
                    i32::from_be_bytes(t.try_into().expect("4 bytes")) as i64
                };
                (time, (indices[i] as usize).min(types.len() - 1))
            })
            .collect();
        let rule = if wide {
            body.get(footer_start..)
                .and_then(|f| f.strip_prefix(b"\n"))
                .and_then(|f| f.iter().position(|&b| b == b'\n').map(|end| &f[..end]))
                .and_then(|f| std::str::from_utf8(f).ok())
                .and_then(PosixRule::parse)
        } else {
            None
        };
        Some(ZoneRules { transitions, types, rule })
    }

    /// What the zone says at `utc` (seconds).
    pub(crate) fn at(&self, utc: i64) -> Offsets {
        let after_table = self.transitions.last().is_none_or(|&(t, _)| utc >= t);
        if after_table && let Some(rule) = &self.rule {
            return rule.at(utc);
        }
        let i = self.transitions.partition_point(|&(t, _)| t <= utc);
        let ty = if i == 0 { &self.types[0] } else { &self.types[self.transitions[i - 1].1] };
        let standard = if ty.dst { self.standard_before(i) } else { ty.offset };
        Offsets { total: ty.offset, standard, dst: ty.dst, abbreviation: ty.abbreviation.clone() }
    }

    /// The offset a local (wall clock) time `wall` (seconds) is at, as Java's `ZoneInfo.getOffsetsByWall`: that of
    /// the last transition whose wall time after it (its instant plus its new offset) isn't after `wall` — a time a
    /// transition skips is taken with the offset before it, one it repeats with the offset after.
    pub(crate) fn offset_by_wall(&self, wall: i64) -> i32 {
        let i = self.transitions.partition_point(|&(t, ty)| t + self.types[ty].offset as i64 <= wall);
        let mut offset = if i == 0 { self.types[0].offset } else { self.types[self.transitions[i - 1].1].offset };
        if let Some(rule) = &self.rule
            && self.transitions.last().is_none_or(|&(t, ty)| wall >= t + self.types[ty].offset as i64)
        {
            match rule.dst {
                None => offset = rule.std_offset,
                Some(ref dst) => {
                    let year = civil_year((wall - rule.std_offset as i64).div_euclid(86_400));
                    let mut changes: Vec<(i64, i32)> = Vec::new();
                    for y in year - 1..=year + 1 {
                        changes.push((rule.instant(&dst.start, y, rule.std_offset), dst.offset));
                        changes.push((rule.instant(&dst.end, y, dst.offset), rule.std_offset));
                    }
                    changes.sort();
                    if let Some(&(_, o)) = changes.iter().rev().find(|&&(t, o)| t + o as i64 <= wall) {
                        offset = o;
                    }
                }
            }
        }
        offset
    }

    /// The zone's offset without daylight saving time now and in the future (Java's `getRawOffset`).
    pub(crate) fn raw_offset(&self) -> i32 {
        if let Some(rule) = &self.rule {
            return rule.std_offset;
        }
        self.standard_before(self.transitions.len())
    }

    /// Whether the zone has daylight saving time from now on (Java's `useDaylightTime`).
    pub(crate) fn uses_daylight_time(&self, now: i64) -> bool {
        match &self.rule {
            Some(rule) => rule.dst.is_some(),
            None => self.transitions.iter().any(|&(t, ty)| t >= now && self.types[ty].dst),
        }
    }

    /// The daylight saving time's amount (Java's `getDSTSavings`): an hour for most zones having it, none without.
    pub(crate) fn dst_savings(&self) -> i32 {
        match &self.rule {
            Some(PosixRule { dst: Some(dst), std_offset, .. }) => dst.offset - std_offset,
            Some(_) => 0,
            None => self
                .transitions
                .iter()
                .enumerate()
                .rev()
                .find(|(_, (_, ty))| self.types[*ty].dst)
                .map_or(0, |(i, (_, ty))| self.types[*ty].offset - self.standard_before(i + 1)),
        }
    }

    /// The standard offset in force before transition `i`: that of the last standard time type before it.
    fn standard_before(&self, i: usize) -> i32 {
        self.transitions[..i]
            .iter()
            .rev()
            .map(|&(_, ty)| &self.types[ty])
            .find(|t| !t.dst)
            .or_else(|| self.types.iter().find(|t| !t.dst))
            .map_or(self.types[0].offset, |t| t.offset)
    }
}

struct Header {
    version: u8,
    isutcnt: usize,
    isstdcnt: usize,
    leapcnt: usize,
    timecnt: usize,
    typecnt: usize,
    charcnt: usize,
}

impl Header {
    fn read(data: &[u8]) -> Option<Header> {
        if data.len() < 44 || &data[..4] != b"TZif" {
            return None;
        }
        let count = |i: usize| u32::from_be_bytes(data[20 + i * 4..24 + i * 4].try_into().expect("4 bytes")) as usize;
        Some(Header {
            version: data[4],
            isutcnt: count(0),
            isstdcnt: count(1),
            leapcnt: count(2),
            timecnt: count(3),
            typecnt: count(4),
            charcnt: count(5),
        })
    }

    /// The length of a data block whose times are `time_len` bytes.
    fn body_len(&self, time_len: usize) -> usize {
        self.timecnt * time_len
            + self.timecnt
            + self.typecnt * 6
            + self.charcnt
            + self.leapcnt * (time_len + 4)
            + self.isstdcnt
            + self.isutcnt
    }
}

/// A POSIX `TZ` rule (`CET-1CEST,M3.5.0,M10.5.0/3`): a standard time and its offset, and a daylight saving time
/// with the dates it starts and ends each year.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct PosixRule {
    std_name: String,
    /// Seconds east of UTC (POSIX writes them west).
    std_offset: i32,
    dst: Option<PosixDst>,
}

#[derive(Clone, Debug, PartialEq)]
struct PosixDst {
    name: String,
    offset: i32,
    start: PosixDate,
    end: PosixDate,
}

/// A day of the year and the local time on it (seconds, may be negative or past a day).
#[derive(Clone, Debug, PartialEq)]
struct PosixDate {
    day: PosixDay,
    time: i32,
}

#[derive(Clone, Debug, PartialEq)]
enum PosixDay {
    /// `Jn`: the n-th day, 1 to 365, February 29 never counted.
    Julian(i32),
    /// `n`: the n-th day, 0 to 365, February 29 counted.
    Zero(i32),
    /// `Mm.w.d`: weekday d (0 Sunday) of week w (5 the last) of month m.
    Month { month: i32, week: i32, weekday: i32 },
}

impl PosixRule {
    pub(crate) fn parse(text: &str) -> Option<PosixRule> {
        let mut p = Cursor { s: text.as_bytes(), at: 0 };
        let std_name = p.name()?;
        let std_offset = -p.offset()?;
        if p.done() {
            return Some(PosixRule { std_name, std_offset, dst: None });
        }
        let name = p.name()?;
        let offset = if p.peek().is_some_and(|c| c == b'+' || c == b'-' || c.is_ascii_digit()) {
            -p.offset()?
        } else {
            std_offset + 3600
        };
        // (no rule: the US's of 2007, as POSIX leaves it to the implementation and glibc does)
        let (start, end) = if p.eat(b',') {
            let start = p.date()?;
            if !p.eat(b',') {
                return None;
            }
            (start, p.date()?)
        } else {
            (
                PosixDate { day: PosixDay::Month { month: 3, week: 2, weekday: 0 }, time: 7200 },
                PosixDate { day: PosixDay::Month { month: 11, week: 1, weekday: 0 }, time: 7200 },
            )
        };
        if !p.done() {
            return None;
        }
        Some(PosixRule { std_name, std_offset, dst: Some(PosixDst { name, offset, start, end }) })
    }

    fn at(&self, utc: i64) -> Offsets {
        let standard = Offsets {
            total: self.std_offset,
            standard: self.std_offset,
            dst: false,
            abbreviation: self.std_name.clone(),
        };
        let Some(dst) = &self.dst else { return standard };
        let year = civil_year((utc + self.std_offset as i64).div_euclid(86_400));
        let start = self.instant(&dst.start, year, self.std_offset);
        let end = self.instant(&dst.end, year, dst.offset);
        let in_dst = if start < end { start <= utc && utc < end } else { !(end <= utc && utc < start) };
        if in_dst {
            Offsets { total: dst.offset, standard: self.std_offset, dst: true, abbreviation: dst.name.clone() }
        } else {
            standard
        }
    }

    /// The UTC instant (seconds) of `date` of `year`, its time read at `offset`.
    fn instant(&self, date: &PosixDate, year: i32, offset: i32) -> i64 {
        posix_instant(date, year, offset)
    }
}

/// The UTC instant (seconds) of `date` of `year`, its time read at `offset` (seconds east of UTC).
fn posix_instant(date: &PosixDate, year: i32, offset: i32) -> i64 {
    let jan1 = days_from_civil(year, 1, 1);
    let day = match date.day {
        PosixDay::Julian(n) => {
            let leap_skip = is_leap(year) && n >= 60;
            jan1 + n as i64 - 1 + leap_skip as i64
        }
        PosixDay::Zero(n) => jan1 + n as i64,
        PosixDay::Month { month, week, weekday } => {
            let first = days_from_civil(year, month, 1);
            // (1970-01-01 was a Thursday, weekday 4)
            let first_weekday = (first + 4).rem_euclid(7) as i32;
            let mut day = first + (weekday - first_weekday).rem_euclid(7) as i64 + 7 * (week as i64 - 1);
            let next_month =
                if month == 12 { days_from_civil(year + 1, 1, 1) } else { days_from_civil(year, month + 1, 1) };
            while day >= next_month {
                day -= 7;
            }
            day
        }
    };
    day * 86_400 + date.time as i64 - offset as i64
}

struct Cursor<'a> {
    s: &'a [u8],
    at: usize,
}

impl Cursor<'_> {
    fn peek(&self) -> Option<u8> {
        self.s.get(self.at).copied()
    }

    fn done(&self) -> bool {
        self.at >= self.s.len()
    }

    fn eat(&mut self, c: u8) -> bool {
        let found = self.peek() == Some(c);
        self.at += found as usize;
        found
    }

    /// `CET`, or `<+03>` quoted.
    fn name(&mut self) -> Option<String> {
        let start = self.at;
        if self.eat(b'<') {
            while self.peek().is_some_and(|c| c != b'>') {
                self.at += 1;
            }
            let name = std::str::from_utf8(&self.s[start + 1..self.at]).ok()?.to_string();
            return self.eat(b'>').then_some(name);
        }
        while self.peek().is_some_and(|c| c.is_ascii_alphabetic()) {
            self.at += 1;
        }
        (self.at - start >= 3).then(|| String::from_utf8_lossy(&self.s[start..self.at]).into_owned())
    }

    fn number(&mut self) -> Option<i32> {
        let start = self.at;
        while self.peek().is_some_and(|c| c.is_ascii_digit()) {
            self.at += 1;
        }
        std::str::from_utf8(&self.s[start..self.at]).ok()?.parse().ok()
    }

    /// `[+-]hh[:mm[:ss]]` in seconds.
    fn offset(&mut self) -> Option<i32> {
        let sign = if self.eat(b'-') {
            -1
        } else {
            self.eat(b'+');
            1
        };
        let mut seconds = self.number()? * 3600;
        if self.eat(b':') {
            seconds += self.number()? * 60;
            if self.eat(b':') {
                seconds += self.number()?;
            }
        }
        Some(sign * seconds)
    }

    /// `Jn`, `n` or `Mm.w.d`, then `/time` (02:00 if none).
    fn date(&mut self) -> Option<PosixDate> {
        let day = if self.eat(b'J') {
            PosixDay::Julian(self.number()?)
        } else if self.eat(b'M') {
            let month = self.number()?;
            if !self.eat(b'.') {
                return None;
            }
            let week = self.number()?;
            if !self.eat(b'.') {
                return None;
            }
            PosixDay::Month { month, week, weekday: self.number()? }
        } else {
            PosixDay::Zero(self.number()?)
        };
        let time = if self.eat(b'/') { self.offset()? } else { 7200 };
        Some(PosixDate { day, time })
    }
}

/// A zone of the system's database by its IANA ID (`Europe/Istanbul`); none if it has none of that name. The
/// platform's database first ([`set_time_zone_database`](crate::environment::set_time_zone_database): the web's),
/// then the system's: Android's tzdata bundle; the TZif files under `TZDIR` and (not on Windows) the usual
/// directories; on Windows its own zones (its registry's, by CLDR's mapping of their names to IANA's, ICU4X's: the
/// zones Windows has, each with its rules year by year — an IANA zone Windows has no zone of its own for, and history
/// older than Windows keeps, aren't there; Windows names its zones, not their abbreviations, so their offsets stand
/// in for them, `+02`, as tzdb's own do for zones without).
pub(crate) fn load(id: &str) -> Option<Arc<ZoneRules>> {
    let valid = !id.is_empty()
        && !id.starts_with('/')
        && id.split('/').all(|part| part != ".." && part != ".")
        && id.chars().all(|c| c.is_ascii_alphanumeric() || matches!(c, '/' | '_' | '-' | '+'));
    if !valid {
        return None;
    }
    if let Some(data) = crate::environment::platform_zone_data(id) {
        return ZoneRules::from_tzif(&data).map(Arc::new);
    }
    #[cfg(target_os = "android")]
    if let Some(rules) = android::load(id) {
        return Some(rules);
    }
    let files = zoneinfo_dirs().into_iter().find_map(|dir| read(&dir.join(id)));
    #[cfg(windows)]
    let files = files.or_else(|| windows::load(id));
    files
}

fn read(path: &Path) -> Option<Arc<ZoneRules>> {
    ZoneRules::from_tzif(&std::fs::read(path).ok()?).map(Arc::new)
}

/// `TZDIR`, then (not on Windows, where those paths name nothing) the usual zoneinfo directories.
fn zoneinfo_dirs() -> Vec<PathBuf> {
    let mut dirs: Vec<PathBuf> =
        crate::environment::var_os("TZDIR").filter(|d| !d.is_empty()).map(PathBuf::from).into_iter().collect();
    if cfg!(not(windows)) {
        dirs.extend(
            ["/usr/share/zoneinfo", "/usr/lib/zoneinfo", "/usr/share/lib/zoneinfo", "/etc/zoneinfo"].map(PathBuf::from),
        );
    }
    dirs
}

/// The zone of a POSIX `TZ` rule itself (`EST5EDT4,M3.2.0,M11.1.0`): no transitions, the rule for all times.
fn posix_zone(tz: &str) -> Option<ZoneRules> {
    let rule = PosixRule::parse(tz)?;
    Some(ZoneRules {
        transitions: Vec::new(),
        types: vec![LocalType { offset: rule.std_offset, dst: false, abbreviation: rule.std_name.clone() }],
        rule: Some(rule),
    })
}

/// The system's zone, its ID and rules, as the JDK finds it: `TZ` (a zone's ID, a TZif file's path, or a POSIX
/// rule), else the platform's word ([`set_system_time_zone`](crate::environment::set_system_time_zone)), else the
/// system's own: on Unix what `/etc/localtime` links to (its ID the path after `zoneinfo/`, macOS's
/// `/var/db/timezone/zoneinfo/…` too), else `/etc/timezone`'s ID; Android's `persist.sys.timezone`; Windows' zone
/// (`GetDynamicTimeZoneInformation`, its name mapped to an IANA ID as the JDK's `tzmappings` does). None if none
/// says.
pub(crate) fn system_zone() -> Option<(String, Arc<ZoneRules>)> {
    if let Some(tz) = crate::environment::var("TZ").filter(|tz| !tz.is_empty()) {
        let tz = tz.strip_prefix(':').unwrap_or(&tz);
        if tz.starts_with('/') {
            let rules = read(Path::new(tz))?;
            return Some((id_of_path(Path::new(tz)).unwrap_or_else(|| tz.to_string()), rules));
        }
        if let Some(rules) = load(tz) {
            return Some((tz.to_string(), rules));
        }
        return Some((tz.to_string(), Arc::new(posix_zone(tz)?)));
    }
    if let Some(id) = crate::environment::platform_time_zone()
        && let Some(rules) = load(&id)
    {
        return Some((id, rules));
    }
    own_system_zone()
}

#[cfg(all(unix, not(target_os = "android")))]
fn own_system_zone() -> Option<(String, Arc<ZoneRules>)> {
    let localtime = Path::new("/etc/localtime");
    if let Ok(target) = std::fs::read_link(localtime) {
        let target = if target.is_relative() { Path::new("/etc").join(target) } else { target };
        if let Some(id) = id_of_path(&target)
            && let Some(rules) = read(localtime)
        {
            return Some((id, rules));
        }
    }
    let id = std::fs::read_to_string("/etc/timezone").ok()?.trim().to_string();
    let rules = load(&id).or_else(|| read(localtime))?;
    Some((id, rules))
}

#[cfg(target_os = "android")]
fn own_system_zone() -> Option<(String, Arc<ZoneRules>)> {
    let id = android::system_zone_id()?;
    let rules = load(&id)?;
    Some((id, rules))
}

#[cfg(windows)]
fn own_system_zone() -> Option<(String, Arc<ZoneRules>)> {
    let (id, posix) = windows::system_zone()?;
    // (a database's rules for the zone where there is one; its rules now as Windows says them otherwise)
    let rules = id.as_deref().and_then(load).or_else(|| posix_zone(&posix).map(Arc::new))?;
    Some((id.unwrap_or(posix), rules))
}

/// None elsewhere (the web: its platform says the zone).
#[cfg(not(any(unix, windows)))]
fn own_system_zone() -> Option<(String, Arc<ZoneRules>)> {
    None
}

/// Android's zone database: the TZif files in one bundle, `tzdata` (bionic's format: a header with the version and
/// the index's and the data's offsets; the index's 52-byte entries, a zone's name and where its TZif bytes are in the
/// data); and the system's zone, its `persist.sys.timezone` property.
#[cfg(any(target_os = "android", test))]
mod android {
    /// The TZif bytes of zone `id` in a bundle.
    pub(super) fn zone_in<'a>(bundle: &'a [u8], id: &str) -> Option<&'a [u8]> {
        let int = |at: usize| Some(u32::from_be_bytes(bundle.get(at..at + 4)?.try_into().ok()?) as usize);
        let (index, data) = (int(12)?, int(16)?);
        let entries = bundle.get(index..data)?;
        entries.as_chunks::<52>().0.iter().find_map(|entry| {
            let name = &entry[..40];
            let name = &name[..name.iter().position(|&b| b == 0).unwrap_or(40)];
            if name != id.as_bytes() {
                return None;
            }
            let start = u32::from_be_bytes(entry[40..44].try_into().ok()?) as usize;
            let length = u32::from_be_bytes(entry[44..48].try_into().ok()?) as usize;
            bundle.get(data.checked_add(start)?..data.checked_add(start)?.checked_add(length)?)
        })
    }

    #[cfg(target_os = "android")]
    pub(super) use device::{load, system_zone_id};

    #[cfg(target_os = "android")]
    mod device {
        #![allow(unsafe_code)]

        use super::super::ZoneRules;
        use std::sync::{Arc, OnceLock};

        /// Where the bundle is: the time zone module's (Android 10 on), then the system image's.
        const BUNDLES: [&str; 3] = [
            "/apex/com.android.tzdata/etc/tz/tzdata",
            "/apex/com.android.runtime/etc/tz/tzdata",
            "/system/usr/share/zoneinfo/tzdata",
        ];

        fn bundle() -> Option<&'static [u8]> {
            static BUNDLE: OnceLock<Option<Vec<u8>>> = OnceLock::new();
            BUNDLE
                .get_or_init(|| BUNDLES.iter().find_map(|p| std::fs::read(p).ok().filter(|b| b.starts_with(b"tzdata"))))
                .as_deref()
        }

        pub(in super::super) fn load(id: &str) -> Option<Arc<ZoneRules>> {
            ZoneRules::from_tzif(super::zone_in(bundle()?, id)?).map(Arc::new)
        }

        /// `persist.sys.timezone`: the zone the user chose.
        pub(in super::super) fn system_zone_id() -> Option<String> {
            // (PROP_VALUE_MAX)
            let mut value = [0u8; 92];
            // SAFETY: the name is a C string; `value` is PROP_VALUE_MAX writable bytes, at most what the call writes
            // (its terminating 0 counted).
            let n = unsafe { libc::__system_property_get(c"persist.sys.timezone".as_ptr(), value.as_mut_ptr().cast()) };
            if n <= 0 {
                return None;
            }
            let bytes = value[..(n as usize).min(value.len())].to_vec();
            String::from_utf8(bytes).ok().filter(|id| !id.is_empty())
        }
    }
}

/// Windows' zone (`GetDynamicTimeZoneInformation`): its IANA ID (CLDR's mapping of Windows' zone names, ICU4X's,
/// the user's region telling apart the zones one name has) and its rules as a POSIX rule; Windows' zones as the
/// system's database (`load`).
#[cfg(windows)]
mod windows {
    #![allow(unsafe_code)]

    use windows_sys::Win32::System::Time::{
        DYNAMIC_TIME_ZONE_INFORMATION, EnumDynamicTimeZoneInformation, GetDynamicTimeZoneInformation,
        GetDynamicTimeZoneInformationEffectiveYears, GetTimeZoneInformationForYear, TIME_ZONE_INFORMATION,
    };

    /// `TIME_ZONE_ID_INVALID`.
    const INVALID: u32 = u32::MAX;

    /// The IANA ID (none if CLDR doesn't map the zone's name) and the POSIX rule.
    pub(super) fn system_zone() -> Option<(Option<String>, String)> {
        // SAFETY: a zeroed DYNAMIC_TIME_ZONE_INFORMATION (integers and arrays of them) is a valid one to fill.
        let mut info: DYNAMIC_TIME_ZONE_INFORMATION = unsafe { std::mem::zeroed() };
        // SAFETY: `info` is a valid structure to write, living through the call.
        if unsafe { GetDynamicTimeZoneInformation(&mut info) } == INVALID {
            return None;
        }
        let key = wide(&info.TimeZoneKeyName);
        let standard = super::WindowsDate::of(&info.StandardDate);
        let daylight = super::WindowsDate::of(&info.DaylightDate);
        let disabled = info.DynamicDaylightTimeDisabled;
        let posix =
            super::windows_posix_rule(info.Bias, info.StandardBias, standard, info.DaylightBias, daylight, disabled);
        Some((iana_id(&key), posix))
    }

    fn iana_id(key: &str) -> Option<String> {
        let region = crate::environment::system_languages()
            .first()
            .and_then(|tag| tag.parse::<icu_locale_core::Locale>().ok())
            .and_then(|l| l.id.region);
        let windows = icu_time::zone::WindowsParser::new();
        let zone = windows.parse(key, region).or_else(|| windows.parse(key, None))?;
        let iana = icu_time::zone::iana::IanaParserExtended::new();
        let found = iana.iter().find(|z| z.time_zone == zone)?;
        Some(found.canonical.to_string())
    }

    fn wide(units: &[u16]) -> String {
        String::from_utf16_lossy(&units[..units.iter().position(|&u| u == 0).unwrap_or(units.len())])
    }

    /// Zone `id` (an IANA ID) as Windows has it: the Windows zone CLDR maps it to (`W. Europe Standard Time` for
    /// `Europe/Berlin`, and for `Europe/Rome` by Italy's region), its rules for each year Windows keeps
    /// (`GetTimeZoneInformationForYear`: its Dynamic DST years, else this year's), the last year's for the years
    /// after. None if Windows has no zone CLDR maps to it.
    pub(super) fn load(id: &str) -> Option<std::sync::Arc<super::ZoneRules>> {
        // (UTC and the fixed `Etc/` zones are the runtime's own, with their names: not Windows' "UTC" zones.)
        if id == "UTC" || id == "GMT" || id.starts_with("Etc/") {
            return None;
        }
        let zone = icu_time::zone::iana::IanaParser::new().parse(id);
        if zone.is_unknown() {
            return None;
        }
        let key = keys().get(zone.0.as_str())?;
        let dynamic = dynamic_zone(key)?;
        let this_year = super::civil_year(crate::clock::unix_time().as_secs().div_euclid(86_400));
        let (mut first, mut last) = (0u32, 0u32);
        // SAFETY: the zone's information lives through the call; the years are written.
        let told = unsafe { GetDynamicTimeZoneInformationEffectiveYears(&dynamic, &mut first, &mut last) } == 0;
        let years = if told && first > 0 && last >= first { first as i32..=last as i32 } else { this_year..=this_year };
        let mut rules = Vec::new();
        for year in years {
            // SAFETY: a zeroed TIME_ZONE_INFORMATION (integers and arrays of them) is a valid one to fill.
            let mut info: TIME_ZONE_INFORMATION = unsafe { std::mem::zeroed() };
            // SAFETY: both structures live through the call; `info` is written.
            if unsafe { GetTimeZoneInformationForYear(year as u16, &dynamic, &mut info) } != 0 {
                rules.push((
                    year,
                    year_of(info.Bias, info.StandardBias, &info.StandardDate, info.DaylightBias, &info.DaylightDate),
                ));
            }
        }
        if rules.is_empty() {
            let d = &dynamic;
            let year = year_of(d.Bias, d.StandardBias, &d.StandardDate, d.DaylightBias, &d.DaylightDate);
            rules.push((this_year, if d.DynamicDaylightTimeDisabled { year.without_daylight() } else { year }));
        }
        super::windows_zone_rules(&rules).map(std::sync::Arc::new)
    }

    fn year_of(
        bias: i32,
        standard_bias: i32,
        standard: &windows_sys::Win32::Foundation::SYSTEMTIME,
        daylight_bias: i32,
        daylight: &windows_sys::Win32::Foundation::SYSTEMTIME,
    ) -> super::WindowsYear {
        super::WindowsYear {
            bias,
            standard_bias,
            standard: super::WindowsDate::of(standard),
            daylight_bias,
            daylight: super::WindowsDate::of(daylight),
        }
    }

    /// Windows' zone of registry key name `key` (`EnumDynamicTimeZoneInformation`'s).
    fn dynamic_zone(key: &str) -> Option<DYNAMIC_TIME_ZONE_INFORMATION> {
        (0..).map_while(enumerate).find(|d| wide(&d.TimeZoneKeyName) == key)
    }

    /// Windows' zone `index` (none past the last).
    fn enumerate(index: u32) -> Option<DYNAMIC_TIME_ZONE_INFORMATION> {
        // SAFETY: a zeroed DYNAMIC_TIME_ZONE_INFORMATION (integers and arrays of them) is a valid one to fill.
        let mut info: DYNAMIC_TIME_ZONE_INFORMATION = unsafe { std::mem::zeroed() };
        // SAFETY: `info` is a valid structure to write, living through the call.
        (unsafe { EnumDynamicTimeZoneInformation(index, &mut info) } == 0).then_some(info)
    }

    /// Windows' zones by the BCP-47 zone CLDR maps them to (each IANA zone's own: `deber`), worked out once: each
    /// zone's name for the world (`001`, the zone's own city) first, then for each region it names another zone
    /// for (`W. Europe Standard Time` for Italy: `itrom`).
    fn keys() -> &'static std::collections::HashMap<String, String> {
        static KEYS: std::sync::OnceLock<std::collections::HashMap<String, String>> = std::sync::OnceLock::new();
        KEYS.get_or_init(|| {
            let names: Vec<String> = (0..).map_while(enumerate).map(|d| wide(&d.TimeZoneKeyName)).collect();
            let windows = icu_time::zone::WindowsParser::new();
            let mut keys = std::collections::HashMap::new();
            for name in &names {
                if let Some(zone) = windows.parse(name, None) {
                    keys.entry(zone.0.as_str().to_string()).or_insert_with(|| name.clone());
                }
            }
            let letters = b'A'..=b'Z';
            let regions: Vec<icu_locale_core::subtags::Region> = letters
                .clone()
                .flat_map(|a| letters.clone().map(move |b| [a, b]))
                .filter_map(|code| icu_locale_core::subtags::Region::try_from_utf8(&code).ok())
                .collect();
            for name in &names {
                for &region in &regions {
                    if let Some(zone) = windows.parse(name, Some(region)) {
                        keys.entry(zone.0.as_str().to_string()).or_insert_with(|| name.clone());
                    }
                }
            }
            keys
        })
    }

    impl super::WindowsDate {
        fn of(t: &windows_sys::Win32::Foundation::SYSTEMTIME) -> super::WindowsDate {
            super::WindowsDate {
                year: t.wYear,
                month: t.wMonth,
                day_of_week: t.wDayOfWeek,
                week: t.wDay,
                hour: t.wHour,
                minute: t.wMinute,
                second: t.wSecond,
            }
        }
    }
}

/// A day Windows' daylight time starts or ends on (its `SYSTEMTIME` as `TIME_ZONE_INFORMATION` has it: weekday
/// `day_of_week`, 0 Sunday, of week `week` of the month, 5 the last, at a local time; `year` 0 for every year).
#[cfg_attr(not(windows), allow(dead_code))]
#[derive(Clone, Copy, Debug, Default)]
struct WindowsDate {
    year: u16,
    month: u16,
    day_of_week: u16,
    week: u16,
    hour: u16,
    minute: u16,
    second: u16,
}

/// The POSIX rule of a Windows zone's biases (minutes west of UTC, as POSIX's offsets) and the days its daylight
/// time starts and ends on (POSIX's `Mm.w.d/time`); its names are its offsets (`<+03>`), as tzdb's for zones without
/// abbreviations.
#[cfg_attr(not(windows), allow(dead_code))]
fn windows_posix_rule(
    bias: i32,
    standard_bias: i32,
    standard: WindowsDate,
    daylight_bias: i32,
    daylight: WindowsDate,
    daylight_disabled: bool,
) -> String {
    let offset = |west: i32| {
        let (sign, m) = if west < 0 { ('-', -west) } else { ('+', west) };
        if m % 60 == 0 { format!("{sign}{}", m / 60) } else { format!("{sign}{}:{:02}", m / 60, m % 60) }
    };
    let name = offset_name;
    let standard_west = bias + standard_bias;
    let mut rule = format!("{}{}", name(standard_west), offset(standard_west));
    // (no daylight time: none set, turned off, or one of a single year: `year` set)
    if daylight_disabled || daylight.month == 0 || standard.month == 0 || daylight.year != 0 {
        return rule;
    }
    let daylight_west = bias + daylight_bias;
    let date = |d: WindowsDate| {
        let time = (d.hour as u32 * 60 + d.minute as u32) * 60 + d.second as u32;
        let (h, m, s) = (time / 3600, time / 60 % 60, time % 60);
        format!("M{}.{}.{}/{h}:{m:02}:{s:02}", d.month, d.week.clamp(1, 5), d.day_of_week)
    };
    rule.push_str(&format!("{}{},{},{}", name(daylight_west), offset(daylight_west), date(daylight), date(standard)));
    rule
}

/// A Windows zone's rules for a year (its `TIME_ZONE_INFORMATION` for that year): its biases (minutes west of UTC)
/// and the days its daylight time starts and ends on.
#[cfg_attr(not(windows), allow(dead_code))]
#[derive(Clone, Copy, Debug, Default)]
struct WindowsYear {
    bias: i32,
    standard_bias: i32,
    standard: WindowsDate,
    daylight_bias: i32,
    daylight: WindowsDate,
}

#[cfg_attr(not(windows), allow(dead_code))]
impl WindowsYear {
    fn has_daylight(&self) -> bool {
        self.daylight.month != 0 && self.standard.month != 0 && self.daylight_bias != self.standard_bias
    }

    fn without_daylight(self) -> WindowsYear {
        WindowsYear { daylight: WindowsDate::default(), ..self }
    }
}

/// The abbreviation tzdb gives a zone's offset where it has none of its own (`+03`, `-0330`), of `west` minutes
/// west.
fn offset_abbreviation(west: i32) -> String {
    let (sign, m) = if west > 0 { ('-', west) } else { ('+', -west) };
    if m % 60 == 0 { format!("{sign}{:02}", m / 60) } else { format!("{sign}{:02}{:02}", m / 60, m % 60) }
}

/// The same as a POSIX rule names it (`<+03>`).
fn offset_name(west: i32) -> String {
    format!("<{}>", offset_abbreviation(west))
}

/// A Windows date of `year` as POSIX's: weekday of week of month, or (a date of one year: `year` set) the day.
#[cfg_attr(not(windows), allow(dead_code))]
fn posix_date_of(d: WindowsDate, year: i32) -> PosixDate {
    let time = (d.hour as i32 * 60 + d.minute as i32) * 60 + d.second as i32;
    let day = if d.year != 0 {
        let n = days_from_civil(year, d.month as i32, d.week as i32) - days_from_civil(year, 1, 1);
        PosixDay::Zero(n as i32)
    } else {
        PosixDay::Month { month: d.month as i32, week: d.week.clamp(1, 5) as i32, weekday: d.day_of_week as i32 }
    };
    PosixDate { day, time }
}

/// The zone of a Windows zone's `years` (each year's rules, in order): its transitions year by year — daylight time
/// starting and ending, the standard offset changing as a year begins —, the first year's rules for the years before
/// it from 1970 (as Windows takes them: its first year's for every year before), the last year's rule after (as a
/// POSIX rule); none for no years.
#[cfg_attr(not(windows), allow(dead_code))]
fn windows_zone_rules(years: &[(i32, WindowsYear)]) -> Option<ZoneRules> {
    let &(_, last) = years.last()?;
    let &(first_year, first) = years.first()?;
    let earlier: Vec<(i32, WindowsYear)> = (1970.min(first_year)..first_year).map(|y| (y, first)).collect();
    let years: Vec<(i32, WindowsYear)> = earlier.into_iter().chain(years.iter().copied()).collect();
    let mut types: Vec<LocalType> = Vec::new();
    let mut transitions: Vec<(i64, usize)> = Vec::new();
    let mut initial = None;
    let mut current: Option<usize> = None;
    for &(year, y) in &years {
        let standard_west = y.bias + y.standard_bias;
        let standard = local_type(&mut types, standard_west, false);
        let mut changes: Vec<(i64, usize)> = Vec::new();
        let mut at_start = standard;
        if y.has_daylight() {
            let daylight_west = y.bias + y.daylight_bias;
            let daylight = local_type(&mut types, daylight_west, true);
            let start = posix_instant(&posix_date_of(y.daylight, year), year, -standard_west * 60);
            let end = posix_instant(&posix_date_of(y.standard, year), year, -daylight_west * 60);
            if start > end {
                at_start = daylight; // (the south: in daylight time as the year begins)
            }
            changes.push((start, daylight));
            changes.push((end, standard));
            changes.sort();
        }
        match current {
            None => initial = Some(at_start),
            // (A new offset from the year's start: its local midnight, at the offset before.)
            Some(before) if before != at_start => {
                transitions.push((days_from_civil(year, 1, 1) * 86_400 - types[before].offset as i64, at_start));
            }
            Some(_) => {}
        }
        current = Some(changes.last().map_or(at_start, |c| c.1));
        transitions.extend(changes);
    }
    // The type before the first transition first (what `at` takes before them).
    let initial = initial?;
    if initial != 0 {
        types.swap(0, initial);
        for t in &mut transitions {
            t.1 = if t.1 == 0 {
                initial
            } else if t.1 == initial {
                0
            } else {
                t.1
            };
        }
    }
    let rule = PosixRule::parse(&windows_posix_rule(
        last.bias,
        last.standard_bias,
        last.standard,
        last.daylight_bias,
        last.daylight,
        !last.has_daylight(),
    ));
    Some(ZoneRules { transitions, types, rule })
}

/// The index of the local type of `west` minutes west of UTC (daylight time or not) in `types`, added if new.
fn local_type(types: &mut Vec<LocalType>, west: i32, dst: bool) -> usize {
    let offset = -west * 60;
    match types.iter().position(|t| t.offset == offset && t.dst == dst) {
        Some(i) => i,
        None => {
            types.push(LocalType { offset, dst, abbreviation: offset_abbreviation(west) });
            types.len() - 1
        }
    }
}

/// The zone ID of a path in a zoneinfo directory (`/usr/share/zoneinfo/Europe/Istanbul` → `Europe/Istanbul`).
fn id_of_path(path: &Path) -> Option<String> {
    let text = path.to_str()?;
    let at = text.rfind("zoneinfo/")?;
    let id = &text[at + "zoneinfo/".len()..];
    // (the `posix/` and `right/` copies of the zones are the same zones)
    let id = id.strip_prefix("posix/").unwrap_or(id);
    (!id.is_empty()).then(|| id.to_string())
}

pub(crate) fn is_leap(year: i32) -> bool {
    year % 4 == 0 && (year % 100 != 0 || year % 400 == 0)
}

/// Days from 1970-01-01 to a proleptic Gregorian date.
pub(crate) fn days_from_civil(year: i32, month: i32, day: i32) -> i64 {
    let (year, month, day) = (year as i64, month as i64, day as i64);
    let y = year - (month <= 2) as i64;
    let era = y.div_euclid(400);
    let yoe = y - era * 400;
    let doy = (153 * (if month > 2 { month - 3 } else { month + 9 }) + 2) / 5 + day - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146_097 + doe - 719_468
}

/// The proleptic Gregorian date (year, month 1–12, day) of days from 1970-01-01.
pub(crate) fn civil_from_days(days: i64) -> (i32, i32, i32) {
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = doy - (153 * mp + 2) / 5 + 1;
    let month = if mp < 10 { mp + 3 } else { mp - 9 };
    ((yoe + era * 400 + (month <= 2) as i64) as i32, month as i32, day as i32)
}

fn civil_year(days: i64) -> i32 {
    civil_from_days(days).0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[cfg_attr(miri, ignore = "reads the system's zoneinfo, which Miri's isolation forbids")]
    fn posix_rules_switch_on_their_days() {
        let berlin = PosixRule::parse("CET-1CEST,M3.5.0,M10.5.0/3").unwrap();
        let utc = |y, m, d, h: i64| days_from_civil(y, m, d) * 86_400 + h * 3600;
        // 2026: summer time from March 29, 01:00 UTC to October 25, 01:00 UTC.
        assert_eq!(berlin.at(utc(2026, 3, 29, 0)).total, 3600);
        assert_eq!(berlin.at(utc(2026, 3, 29, 1)).total, 7200);
        assert_eq!(berlin.at(utc(2026, 10, 25, 0)).abbreviation, "CEST");
        assert_eq!(berlin.at(utc(2026, 10, 25, 1)).abbreviation, "CET");
        let sydney = PosixRule::parse("AEST-10AEDT,M10.1.0,M4.1.0/3").unwrap();
        assert!(sydney.at(utc(2026, 1, 10, 0)).dst);
        assert!(!sydney.at(utc(2026, 6, 10, 0)).dst);
        let istanbul = PosixRule::parse("<+03>-3").unwrap();
        assert_eq!((istanbul.at(0).total, istanbul.at(0).abbreviation.as_str()), (10_800, "+03"));
        assert_eq!(PosixRule::parse("EST5EDT").unwrap().at(utc(2026, 7, 1, 12)).total, -4 * 3600);
        assert!(PosixRule::parse("C").is_none());
    }

    #[test]
    #[cfg_attr(miri, ignore = "reads the system's zoneinfo, which Miri's isolation forbids")]
    fn wall_times_take_javas_offsets() {
        let ny = ZoneRules {
            transitions: Vec::new(),
            types: vec![LocalType { offset: -18_000, dst: false, abbreviation: "EST".into() }],
            rule: PosixRule::parse("EST5EDT,M3.2.0,M11.1.0"),
        };
        let wall = |y, m, d, h: i64, min: i64| days_from_civil(y, m, d) * 86_400 + h * 3600 + min * 60;
        // 02:30 on March 8, 2026 doesn't exist: the offset before (03:30 EDT); 01:30 on November 1 is twice: the
        // offset after (EST).
        assert_eq!(ny.offset_by_wall(wall(2026, 3, 8, 2, 30)), -18_000);
        assert_eq!(ny.offset_by_wall(wall(2026, 3, 8, 3, 30)), -14_400);
        assert_eq!(ny.offset_by_wall(wall(2026, 11, 1, 1, 30)), -18_000);
        assert_eq!(ny.offset_by_wall(wall(2026, 7, 1, 12, 0)), -14_400);
        assert_eq!(ny.raw_offset(), -18_000);
        assert_eq!(ny.dst_savings(), 3600);
    }

    #[test]
    #[cfg_attr(miri, ignore = "reads the system's zoneinfo, which Miri's isolation forbids")]
    fn the_systems_zones_are_read() {
        // (where the system has its zoneinfo database)
        let Some(berlin) = load("Europe/Berlin") else { return };
        let utc = |y, m, d| days_from_civil(y, m, d) * 86_400;
        assert_eq!(berlin.at(utc(2026, 1, 15)).total, 3600);
        // (Windows' zones have no abbreviations: their offsets stand in.)
        assert_eq!(berlin.at(utc(2026, 7, 15)).abbreviation, if cfg!(windows) { "+02" } else { "CEST" });
        assert_eq!(berlin.at(utc(1990, 7, 15)).total, 7200);
        assert_eq!(berlin.raw_offset(), 3600);
        assert!(load("../etc/passwd").is_none() && load("Nowhere/Never").is_none());
    }

    #[test]
    fn windows_zones_are_posix_rules() {
        let date = |month, week, hour| WindowsDate { month, week, hour, ..WindowsDate::default() };
        // Pacific Standard Time: 8 hours west, an hour's daylight time from March's second Sunday to November's first.
        let pacific = windows_posix_rule(480, 0, date(11, 1, 2), -60, date(3, 2, 2), false);
        assert_eq!(pacific, "<-08>+8<-07>+7,M3.2.0/2:00:00,M11.1.0/2:00:00");
        let pacific = PosixRule::parse(&pacific).unwrap();
        let utc = |y, m, d, h: i64| days_from_civil(y, m, d) * 86_400 + h * 3600;
        assert_eq!(pacific.at(utc(2026, 1, 10, 12)).total, -8 * 3600);
        assert_eq!(pacific.at(utc(2026, 7, 10, 12)).total, -7 * 3600);
        // Turkey Standard Time: 3 hours east, no daylight time; India's half hour; daylight time turned off.
        assert_eq!(windows_posix_rule(-180, 0, date(0, 0, 0), -60, date(0, 0, 0), false), "<+03>-3");
        assert_eq!(windows_posix_rule(-330, 0, date(0, 0, 0), 0, date(0, 0, 0), false), "<+0530>-5:30");
        assert_eq!(windows_posix_rule(480, 0, date(11, 1, 2), -60, date(3, 2, 2), true), "<-08>+8");
        assert_eq!(PosixRule::parse("<+0530>-5:30").unwrap().at(0).total, 19_800);
    }

    #[test]
    fn windows_zones_are_ruled_year_by_year() {
        let date =
            |month, week, day_of_week, hour| WindowsDate { month, week, day_of_week, hour, ..WindowsDate::default() };
        let utc = |y, m, d, h: i64| days_from_civil(y, m, d) * 86_400 + h * 3600;
        // Pacific Standard Time: its rule before 2007 (April's first Sunday to October's last) and after.
        let year =
            |standard, daylight| WindowsYear { bias: 480, standard_bias: 0, standard, daylight_bias: -60, daylight };
        let pacific = windows_zone_rules(&[
            (2006, year(date(10, 5, 0, 2), date(4, 1, 0, 2))),
            (2007, year(date(11, 1, 0, 2), date(3, 2, 0, 2))),
        ])
        .unwrap();
        // (Before its years, from 1970: the first one's rule, as Windows takes it; before 1970 its standard time.)
        assert_eq!(pacific.at(utc(2005, 7, 1, 0)).total, -7 * 3600);
        assert_eq!(pacific.at(utc(1975, 1, 1, 0)).total, -8 * 3600);
        assert_eq!(pacific.at(utc(1969, 7, 1, 0)).total, -8 * 3600);
        assert_eq!(pacific.at(utc(2006, 3, 20, 12)).total, -8 * 3600);
        assert_eq!(pacific.at(utc(2006, 4, 3, 12)).total, -7 * 3600);
        assert_eq!(pacific.at(utc(2006, 10, 30, 12)).total, -8 * 3600);
        assert_eq!(pacific.at(utc(2007, 3, 20, 12)).total, -7 * 3600); // (the new rule)
        assert_eq!(pacific.at(utc(2030, 7, 1, 0)).abbreviation, "-07"); // (after: the last year's rule)
        assert_eq!(pacific.at(utc(2030, 12, 1, 0)).total, -8 * 3600);
        // The south: daylight time from October's first Sunday to April's, as the year begins too.
        let sydney = windows_zone_rules(&[(
            2026,
            WindowsYear {
                bias: -600,
                standard_bias: 0,
                standard: date(4, 1, 0, 3),
                daylight_bias: -60,
                daylight: date(10, 1, 0, 2),
            },
        )])
        .unwrap();
        assert_eq!(sydney.at(utc(2026, 1, 15, 0)).total, 11 * 3600);
        assert_eq!(sydney.at(utc(2026, 6, 15, 0)).total, 10 * 3600);
        assert_eq!(sydney.at(utc(2026, 12, 15, 0)).total, 11 * 3600);
        // A standard offset changing as a year begins (Turkey's +2 with daylight time, then +3 without).
        let none = WindowsDate::default();
        let istanbul = windows_zone_rules(&[
            (
                2015,
                WindowsYear {
                    bias: -120,
                    standard_bias: 0,
                    standard: date(10, 5, 0, 4),
                    daylight_bias: -60,
                    daylight: date(3, 5, 0, 3),
                },
            ),
            (2017, WindowsYear { bias: -180, standard_bias: 0, standard: none, daylight_bias: 0, daylight: none }),
        ])
        .unwrap();
        assert_eq!(istanbul.at(utc(2015, 12, 31, 21)).total, 2 * 3600);
        assert_eq!(istanbul.at(utc(2017, 1, 1, 0)).total, 3 * 3600);
        assert_eq!(istanbul.at(utc(2026, 7, 1, 0)).total, 3 * 3600);
        assert_eq!(istanbul.raw_offset(), 3 * 3600);
        assert!(windows_zone_rules(&[]).is_none());
    }

    #[test]
    #[cfg(windows)]
    fn windows_has_the_zones_cldr_maps() {
        let utc = |y, m, d| days_from_civil(y, m, d) * 86_400;
        // (Every Windows has these zones; wine's registry has them too.)
        let berlin = load("Europe/Berlin").expect("W. Europe Standard Time");
        assert_eq!(berlin.at(utc(2026, 1, 15)).total, 3600);
        assert_eq!(berlin.at(utc(2026, 7, 15)).total, 7200);
        let tokyo = load("Asia/Tokyo").expect("Tokyo Standard Time");
        assert_eq!(tokyo.at(utc(2026, 7, 15)).total, 9 * 3600);
        assert!(load("Nowhere/Never").is_none());
    }

    #[test]
    #[cfg(windows)]
    fn windows_says_its_zone() {
        let (id, posix) = windows::system_zone().expect("Windows' zone");
        assert!(PosixRule::parse(&posix).is_some(), "{posix}");
        assert!(id.as_deref().is_none_or(|id| !id.is_empty() && !id.contains(' ')), "{id:?}");
        let (system, rules) = system_zone().expect("the system's zone");
        let now = crate::clock::unix_time().as_secs();
        assert_eq!(rules.at(now).total, posix_zone(&posix).unwrap().at(now).total, "{system}");
    }

    #[test]
    fn androids_bundle_has_its_zones() {
        // A bundle of two zones: the header (12 bytes of version, the index's, the data's and the final offsets),
        // the index's entries (a 40-byte name, the start in the data, the length, the raw offset), the data.
        let zones: [(&str, &[u8]); 2] = [("Europe/Istanbul", b"TZif-istanbul"), ("UTC", b"TZif-utc")];
        let mut index = Vec::new();
        let mut data = Vec::new();
        for (name, bytes) in zones {
            let mut entry = [0u8; 52];
            entry[..name.len()].copy_from_slice(name.as_bytes());
            entry[40..44].copy_from_slice(&(data.len() as u32).to_be_bytes());
            entry[44..48].copy_from_slice(&(bytes.len() as u32).to_be_bytes());
            index.extend_from_slice(&entry);
            data.extend_from_slice(bytes);
        }
        let mut bundle = b"tzdata2026a\0".to_vec();
        let data_at = 24 + index.len() as u32;
        for offset in [24, data_at, data_at + data.len() as u32] {
            bundle.extend_from_slice(&u32::to_be_bytes(offset));
        }
        bundle.extend_from_slice(&index);
        bundle.extend_from_slice(&data);
        assert_eq!(android::zone_in(&bundle, "UTC"), Some(&b"TZif-utc"[..]));
        assert_eq!(android::zone_in(&bundle, "Europe/Istanbul"), Some(&b"TZif-istanbul"[..]));
        assert_eq!(android::zone_in(&bundle, "Europe"), None);
        assert_eq!(android::zone_in(&bundle[..30], "UTC"), None);
    }
}
