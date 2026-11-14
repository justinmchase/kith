use super::value::{instant_json, instant_millis, whole_number};
use crate::error::Error;
use serde_json::{json, Value as Json};
use time::OffsetDateTime;
use uffda_runtime::codec::data::{DataDocument, DataLimits, DataNode};

pub fn call(name: &str, args: &[Json]) -> Result<Option<Json>, Error> {
    let value = match name {
        "instant" => {
            expect_arity(name, args, 1)?;
            let text = args[0]
                .as_str()
                .ok_or_else(|| Error::new("instant expected RFC3339 text"))?;
            instant_json(parse_instant(text)?)
        }
        "instant_text" => {
            expect_arity(name, args, 1)?;
            json!(instant_text(expect_instant("instant_text", &args[0])?)?)
        }
        "zone" => {
            expect_arity(name, args, 1)?;
            let id = args[0]
                .as_str()
                .ok_or_else(|| Error::new("zone expected a zone id"))?;
            zone_value(id)?
        }
        "date_time" => {
            expect_arity(name, args, 2)?;
            let instant = expect_instant("date_time", &args[0])?;
            let zone = expect_zone("date_time", &args[1])?;
            date_time_json(&date_time(instant, &zone)?)
        }
        "instant_at" => {
            expect_arity(name, args, 2)?;
            let date = expect_date_time(&args[0])?;
            let zone = expect_zone("instant_at", &args[1])?;
            instant_json(instant_at(&date, &zone)?)
        }
        "add_seconds" => {
            expect_arity(name, args, 2)?;
            let instant = expect_instant("add_seconds", &args[0])?;
            let seconds = args[1]
                .as_f64()
                .filter(|number| number.is_finite())
                .ok_or_else(|| Error::new("add_seconds expected a number"))?;
            let next = instant
                .checked_add(seconds_to_millis(seconds)?)
                .ok_or_else(|| Error::new("add_seconds overflow"))?;
            instant_json(next)
        }
        "diff_seconds" => {
            expect_arity(name, args, 2)?;
            let later = expect_instant("diff_seconds", &args[0])?;
            let earlier = expect_instant("diff_seconds", &args[1])?;
            json!((later - earlier) as f64 / 1000.0)
        }
        "offset_seconds" => {
            expect_arity(name, args, 2)?;
            let zone = expect_zone("offset_seconds", &args[0])?;
            let instant = expect_instant("offset_seconds", &args[1])?;
            json!(offset_seconds(&zone, instant)? as f64)
        }
        _ => return Ok(None),
    };
    Ok(Some(value))
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Zone {
    Utc,
    Fixed { seconds: i64 },
    Iana { id: String },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct DateTime {
    pub year: i32,
    pub month: u32,
    pub day: u32,
    pub hour: u32,
    pub minute: u32,
    pub second: u32,
    pub millisecond: u32,
}

pub fn utc_zone() -> Json {
    json!({ "kind": "utc" })
}

pub fn zone_value(id: &str) -> Result<Json, Error> {
    Ok(zone_json(&parse_zone(id)?))
}

pub fn parse_zone(id: &str) -> Result<Zone, Error> {
    if id == "UTC" {
        return Ok(Zone::Utc);
    }
    if let Some(seconds) = fixed_offset(id) {
        return Ok(Zone::Fixed { seconds });
    }
    if matches!(id, "America/New_York" | "Europe/London" | "Asia/Tokyo") {
        return Ok(Zone::Iana { id: id.to_string() });
    }
    Err(Error::new("invalid zone"))
}

pub fn zone_of(value: &Json) -> Option<Zone> {
    let object = value.as_object()?;
    match object.get("kind")?.as_str()? {
        "utc" => Some(Zone::Utc),
        "fixed" => {
            let seconds = whole_number(object.get("seconds")?)?;
            Some(Zone::Fixed { seconds })
        }
        "iana" => {
            let id = object.get("id")?.as_str()?;
            if matches!(id, "America/New_York" | "Europe/London" | "Asia/Tokyo") {
                Some(Zone::Iana { id: id.to_string() })
            } else {
                None
            }
        }
        _ => None,
    }
}

pub fn date_time_of(value: &Json) -> Option<DateTime> {
    let object = value.as_object()?;
    let year = i32::try_from(whole_number(object.get("year")?)?).ok()?;
    let month = u32::try_from(whole_number(object.get("month")?)?).ok()?;
    let day = u32::try_from(whole_number(object.get("day")?)?).ok()?;
    let hour = u32::try_from(whole_number(object.get("hour")?)?).ok()?;
    let minute = u32::try_from(whole_number(object.get("minute")?)?).ok()?;
    let second = u32::try_from(whole_number(object.get("second")?)?).ok()?;
    let millisecond = u32::try_from(whole_number(object.get("millisecond")?)?).ok()?;
    let date = DateTime {
        year,
        month,
        day,
        hour,
        minute,
        second,
        millisecond,
    };
    valid_date(&date).then_some(date)
}

pub fn process_zone() -> Json {
    match std::env::var("TZ") {
        Ok(value) => zone_value(&value).unwrap_or_else(|_| utc_zone()),
        Err(_) => utc_zone(),
    }
}

pub fn parse_instant(text: &str) -> Result<i64, Error> {
    let source = format!("@date\"{text}\"");
    let document = DataDocument::from_codec(&source).map_err(|_| Error::new("invalid instant"))?;
    let DataNode::Date(value) = document
        .nodes()
        .get(document.root())
        .ok_or_else(|| Error::new("invalid instant"))?
    else {
        return Err(Error::new("invalid instant"));
    };
    let nanos = value.unix_timestamp_nanos();
    if nanos % 1_000_000 != 0 {
        return Err(Error::new("invalid instant"));
    }
    i64::try_from(nanos / 1_000_000).map_err(|_| Error::new("invalid instant"))
}

pub fn instant_text(millis: i64) -> Result<String, Error> {
    let date = OffsetDateTime::from_unix_timestamp_nanos(i128::from(millis) * 1_000_000)
        .map_err(|_| Error::new("invalid instant"))?;
    let document = DataDocument::from_nodes(vec![DataNode::Date(date)], 0, DataLimits::default())
        .map_err(|error| Error::new(error.to_string()))?;
    let text = document
        .encode(false, DataLimits::default())
        .map_err(|error| Error::new(error.to_string()))?;
    text.strip_prefix("@date\"")
        .and_then(|text| text.strip_suffix('"'))
        .map(str::to_string)
        .ok_or_else(|| Error::new("invalid instant"))
}

pub fn wall_millis() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .ok()
        .and_then(|duration| i64::try_from(duration.as_millis()).ok())
        .unwrap_or(0)
}

pub fn seconds_to_millis(seconds: f64) -> Result<i64, Error> {
    if !seconds.is_finite() {
        return Err(Error::new("add_seconds expected a number"));
    }
    let millis = (seconds * 1000.0).round();
    if !millis.is_finite() || millis.abs() > (1_i64 << 53) as f64 {
        return Err(Error::new("add_seconds overflow"));
    }
    Ok(millis as i64)
}

fn date_time(millis: i64, zone: &Zone) -> Result<DateTime, Error> {
    let offset = offset_seconds(zone, millis)?;
    let local = offset
        .checked_mul(1000)
        .and_then(|delta| millis.checked_add(delta))
        .ok_or_else(|| Error::new("date_time overflow"))?;
    Ok(civil_from_millis(local))
}

fn instant_at(date: &DateTime, zone: &Zone) -> Result<i64, Error> {
    let as_utc = civil_to_millis(date)?;
    match zone {
        Zone::Utc => Ok(as_utc),
        Zone::Fixed { seconds } => {
            let delta = seconds
                .checked_mul(1000)
                .ok_or_else(|| Error::new("instant_at overflow"))?;
            as_utc
                .checked_sub(delta)
                .ok_or_else(|| Error::new("instant_at overflow"))
        }
        Zone::Iana { id } => {
            let standard = standard_offset(id);
            let daylight = daylight_offset(id);
            let mut found = None;
            let offsets = if daylight == standard {
                vec![standard]
            } else {
                vec![standard, daylight]
            };
            for offset in offsets {
                let Some(delta) = offset.checked_mul(1000) else {
                    continue;
                };
                let Some(candidate) = as_utc.checked_sub(delta) else {
                    continue;
                };
                if offset_seconds(zone, candidate)? == offset {
                    if found.is_some() {
                        return Err(Error::new("overlap"));
                    }
                    found = Some(candidate);
                }
            }
            found.ok_or_else(|| Error::new("gap"))
        }
    }
}

pub fn offset_seconds(zone: &Zone, millis: i64) -> Result<i64, Error> {
    Ok(match zone {
        Zone::Utc => 0,
        Zone::Fixed { seconds } => *seconds,
        Zone::Iana { id } => {
            if id == "Asia/Tokyo" {
                9 * 3600
            } else if in_daylight(id, millis) {
                daylight_offset(id)
            } else {
                standard_offset(id)
            }
        }
    })
}

fn in_daylight(id: &str, millis: i64) -> bool {
    let year = civil_from_millis(millis).year;
    (year - 1..=year + 1).any(|year| {
        let (start, end) = daylight_bounds(id, year);
        millis >= start && millis < end
    })
}

fn daylight_bounds(id: &str, year: i32) -> (i64, i64) {
    if id == "America/New_York" {
        let start = civil_to_millis(&DateTime {
            year,
            month: 3,
            day: nth_sunday(year, 3, 2),
            hour: 7,
            minute: 0,
            second: 0,
            millisecond: 0,
        })
        .unwrap_or(i64::MAX);
        let end = civil_to_millis(&DateTime {
            year,
            month: 11,
            day: nth_sunday(year, 11, 1),
            hour: 6,
            minute: 0,
            second: 0,
            millisecond: 0,
        })
        .unwrap_or(i64::MIN);
        (start, end)
    } else if id == "Europe/London" {
        let start = civil_to_millis(&DateTime {
            year,
            month: 3,
            day: last_sunday(year, 3),
            hour: 1,
            minute: 0,
            second: 0,
            millisecond: 0,
        })
        .unwrap_or(i64::MAX);
        let end = civil_to_millis(&DateTime {
            year,
            month: 10,
            day: last_sunday(year, 10),
            hour: 1,
            minute: 0,
            second: 0,
            millisecond: 0,
        })
        .unwrap_or(i64::MIN);
        (start, end)
    } else {
        (i64::MAX, i64::MIN)
    }
}

fn standard_offset(id: &str) -> i64 {
    match id {
        "America/New_York" => -5 * 3600,
        "Europe/London" => 0,
        "Asia/Tokyo" => 9 * 3600,
        _ => 0,
    }
}

fn daylight_offset(id: &str) -> i64 {
    match id {
        "America/New_York" => -4 * 3600,
        "Europe/London" => 3600,
        _ => standard_offset(id),
    }
}

fn fixed_offset(text: &str) -> Option<i64> {
    let bytes = text.as_bytes();
    if bytes.len() != 6 || (bytes[0] != b'+' && bytes[0] != b'-') || bytes[3] != b':' {
        return None;
    }
    let hours = std::str::from_utf8(&bytes[1..3])
        .ok()?
        .parse::<i64>()
        .ok()?;
    let minutes = std::str::from_utf8(&bytes[4..6])
        .ok()?
        .parse::<i64>()
        .ok()?;
    if !(0..=23).contains(&hours) || !(0..=59).contains(&minutes) {
        return None;
    }
    let seconds = hours * 3600 + minutes * 60;
    Some(if bytes[0] == b'-' { -seconds } else { seconds })
}

fn zone_json(zone: &Zone) -> Json {
    match zone {
        Zone::Utc => utc_zone(),
        Zone::Fixed { seconds } => json!({ "kind": "fixed", "seconds": *seconds }),
        Zone::Iana { id } => json!({ "kind": "iana", "id": id }),
    }
}

fn date_time_json(date: &DateTime) -> Json {
    json!({
        "year": date.year,
        "month": date.month,
        "day": date.day,
        "hour": date.hour,
        "minute": date.minute,
        "second": date.second,
        "millisecond": date.millisecond,
    })
}

fn valid_date(date: &DateTime) -> bool {
    (-9999..=9999).contains(&date.year)
        && (1..=12).contains(&date.month)
        && date.day >= 1
        && date.day <= days_in_month(date.year, date.month)
        && date.hour <= 23
        && date.minute <= 59
        && date.second <= 59
        && date.millisecond <= 999
}

fn days_in_month(year: i32, month: u32) -> u32 {
    match month {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        2 if is_leap(year) => 29,
        2 => 28,
        _ => 0,
    }
}

fn is_leap(year: i32) -> bool {
    year % 4 == 0 && (year % 100 != 0 || year % 400 == 0)
}

fn civil_from_millis(millis: i64) -> DateTime {
    let day = millis.div_euclid(86_400_000);
    let tod = millis.rem_euclid(86_400_000) as u32;
    let (year, month, day) = civil_from_days(day);
    DateTime {
        year,
        month,
        day,
        hour: tod / 3_600_000,
        minute: (tod / 60_000) % 60,
        second: (tod / 1_000) % 60,
        millisecond: tod % 1_000,
    }
}

fn civil_to_millis(date: &DateTime) -> Result<i64, Error> {
    if !valid_date(date) {
        return Err(Error::new("invalid date"));
    }
    let days = days_from_civil(date.year, date.month, date.day);
    let tod = i64::from(date.hour) * 3_600_000
        + i64::from(date.minute) * 60_000
        + i64::from(date.second) * 1_000
        + i64::from(date.millisecond);
    days.checked_mul(86_400_000)
        .and_then(|day| day.checked_add(tod))
        .ok_or_else(|| Error::new("instant_at overflow"))
}

fn days_from_civil(year: i32, month: u32, day: u32) -> i64 {
    let year = if month <= 2 { year - 1 } else { year } as i64;
    let era = if year >= 0 { year } else { year - 399 } / 400;
    let yoe = (year - era * 400) as u64;
    let month_prime = if month > 2 { month - 3 } else { month + 9 };
    let doy = (153 * month_prime + 2) / 5 + day - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + u64::from(doy);
    era * 146097 + doe as i64 - 719468
}

fn civil_from_days(days: i64) -> (i32, u32, u32) {
    let z = days + 719468;
    let era = if z >= 0 { z } else { z - 146096 } / 146097;
    let doe = (z - era * 146097) as u64;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
    let y = yoe as i64 + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = doy - (153 * mp + 2) / 5 + 1;
    let month = if mp < 10 { mp + 3 } else { mp - 9 };
    let year = if month <= 2 { y + 1 } else { y };
    (year as i32, month as u32, day as u32)
}

fn weekday_sunday0(year: i32, month: u32, day: u32) -> u32 {
    let delta = days_from_civil(year, month, day) - days_from_civil(1970, 1, 1);
    ((delta % 7 + 7) % 7 + 4) as u32 % 7
}

fn nth_sunday(year: i32, month: u32, n: u32) -> u32 {
    let first = weekday_sunday0(year, month, 1);
    1 + (7 - first) % 7 + (n - 1) * 7
}

fn last_sunday(year: i32, month: u32) -> u32 {
    let last = days_in_month(year, month);
    last - weekday_sunday0(year, month, last)
}

fn expect_arity(name: &str, args: &[Json], arity: usize) -> Result<(), Error> {
    if args.len() == arity {
        Ok(())
    } else {
        Err(Error::new(format!("{name} expected {arity} arguments")))
    }
}

fn expect_instant(name: &str, value: &Json) -> Result<i64, Error> {
    instant_millis(value).ok_or_else(|| Error::new(format!("{name} expected an instant")))
}

fn expect_zone(name: &str, value: &Json) -> Result<Zone, Error> {
    zone_of(value).ok_or_else(|| Error::new(format!("{name} expected a zone")))
}

fn expect_date_time(value: &Json) -> Result<DateTime, Error> {
    date_time_of(value).ok_or_else(|| Error::new("instant_at expected a date"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    // req:builtins-004
    fn poc_zones_show_a_date_line_a_gap_and_an_overlap() {
        let utc = parse_instant("2024-01-01T00:00:00.000Z").unwrap();
        let new_york = zone_of(&zone_value("America/New_York").unwrap()).unwrap();
        let tokyo = zone_of(&zone_value("Asia/Tokyo").unwrap()).unwrap();
        let ny = date_time(utc, &new_york).unwrap();
        assert_eq!((ny.year, ny.month, ny.day, ny.hour), (2023, 12, 31, 19));
        let tk = date_time(utc, &tokyo).unwrap();
        assert_eq!((tk.year, tk.month, tk.day, tk.hour), (2024, 1, 1, 9));
        assert_eq!(offset_seconds(&new_york, utc).unwrap(), -5 * 3600);
        let july = parse_instant("2024-07-01T12:00:00.000Z").unwrap();
        assert_eq!(offset_seconds(&new_york, july).unwrap(), -4 * 3600);
        assert_eq!(offset_seconds(&tokyo, utc).unwrap(), 9 * 3600);

        let gap = DateTime {
            year: 2024,
            month: 3,
            day: 10,
            hour: 2,
            minute: 30,
            second: 0,
            millisecond: 0,
        };
        assert_eq!(instant_at(&gap, &new_york).unwrap_err().message, "gap");
        let overlap = DateTime {
            year: 2024,
            month: 11,
            day: 3,
            hour: 1,
            minute: 30,
            second: 0,
            millisecond: 0,
        };
        assert_eq!(
            instant_at(&overlap, &new_york).unwrap_err().message,
            "overlap"
        );
        let winter = DateTime {
            year: 2024,
            month: 1,
            day: 1,
            hour: 19,
            minute: 0,
            second: 0,
            millisecond: 0,
        };
        assert_eq!(
            instant_text(instant_at(&winter, &new_york).unwrap()).unwrap(),
            "2024-01-02T00:00:00Z"
        );
        assert_eq!(seconds_to_millis(1.5).unwrap(), 1500);
        assert_eq!(seconds_to_millis(1.0005).unwrap(), 1001);
        assert_eq!(seconds_to_millis(-1.0005).unwrap(), -1001);
        assert!(date_time_of(&json!({
            "year": 2023, "month": 2, "day": 29,
            "hour": 0, "minute": 0, "second": 0, "millisecond": 0
        }))
        .is_none());
        assert!(date_time_of(&json!({
            "year": 2024, "month": 2, "day": 29,
            "hour": 0, "minute": 0, "second": 0, "millisecond": 0
        }))
        .is_some());
    }
}
