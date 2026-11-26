//! The system's time zone database (what the JDK's `ZoneInfo` reads from its own `tzdb.dat`): the IANA zones of the
//! TZif files under `TZDIR`, `/usr/share/zoneinfo` (RFC 8536: their transitions, then the POSIX `TZ` rule of their
//! footer for the times after the last), and which zone the system is in (`TZ`, else `/etc/localtime`).

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

/// A zone of the system's database by its IANA ID (`Europe/Istanbul`); none if it has none of that name.
pub(crate) fn load(id: &str) -> Option<Arc<ZoneRules>> {
    let valid = !id.is_empty()
        && !id.starts_with('/')
        && id.split('/').all(|part| part != ".." && part != ".")
        && id.chars().all(|c| c.is_ascii_alphanumeric() || matches!(c, '/' | '_' | '-' | '+'));
    if !valid {
        return None;
    }
    zoneinfo_dirs().into_iter().find_map(|dir| read(&dir.join(id)))
}

fn read(path: &Path) -> Option<Arc<ZoneRules>> {
    ZoneRules::from_tzif(&std::fs::read(path).ok()?).map(Arc::new)
}

fn zoneinfo_dirs() -> Vec<PathBuf> {
    let mut dirs: Vec<PathBuf> =
        std::env::var_os("TZDIR").filter(|d| !d.is_empty()).map(PathBuf::from).into_iter().collect();
    dirs.extend(
        ["/usr/share/zoneinfo", "/usr/lib/zoneinfo", "/usr/share/lib/zoneinfo", "/etc/zoneinfo"].map(PathBuf::from),
    );
    dirs
}

/// The system's zone, its ID and rules, as the JDK finds it on Unix: `TZ` (a zone's ID, or a TZif file's path), else
/// what `/etc/localtime` links to (its ID the path after `zoneinfo/`), else `/etc/timezone`'s ID; none if the system
/// says none.
pub(crate) fn system_zone() -> Option<(String, Arc<ZoneRules>)> {
    if let Some(tz) = std::env::var("TZ").ok().filter(|tz| !tz.is_empty()) {
        let tz = tz.strip_prefix(':').unwrap_or(&tz);
        if tz.starts_with('/') {
            let rules = read(Path::new(tz))?;
            return Some((id_of_path(Path::new(tz)).unwrap_or_else(|| tz.to_string()), rules));
        }
        if let Some(rules) = load(tz) {
            return Some((tz.to_string(), rules));
        }
        // (a POSIX rule itself, `EST5EDT4,M3.2.0,M11.1.0`)
        let rule = PosixRule::parse(tz)?;
        let rules = ZoneRules {
            transitions: Vec::new(),
            types: vec![LocalType { offset: rule.std_offset, dst: false, abbreviation: rule.std_name.clone() }],
            rule: Some(rule),
        };
        return Some((tz.to_string(), Arc::new(rules)));
    }
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
    fn the_systems_zones_are_read() {
        // (where the system has its zoneinfo database)
        let Some(berlin) = load("Europe/Berlin") else { return };
        let utc = |y, m, d| days_from_civil(y, m, d) * 86_400;
        assert_eq!(berlin.at(utc(2026, 1, 15)).total, 3600);
        assert_eq!(berlin.at(utc(2026, 7, 15)).abbreviation, "CEST");
        assert_eq!(berlin.at(utc(1990, 7, 15)).total, 7200);
        assert_eq!(berlin.raw_offset(), 3600);
        assert!(load("../etc/passwd").is_none() && load("Nowhere/Never").is_none());
    }
}
