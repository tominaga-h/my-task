use chrono::{Datelike, Local, NaiveDate};

/// ざっくりした日付文字列を NaiveDate に変換する。
/// 対応形式:
///   - YYYY-MM-DD (例: 2026-04-15)
///   - 今日 / today
///   - 明日 / tomorrow
///   - 明後日
///   - 来週 / next week (7日後)
///   - 来月 / next month
///   - 月曜〜日曜 / mon〜sun (次のその曜日)
///     返り値が None の場合はパース失敗。
pub fn parse_fuzzy_date(input: &str) -> Option<NaiveDate> {
    let today = Local::now().date_naive();
    let s = input.trim().to_lowercase();

    // YYYY-MM-DD
    if let Ok(d) = NaiveDate::parse_from_str(&s, "%Y-%m-%d") {
        return Some(d);
    }

    match s.as_str() {
        "今日" | "today" => Some(today),
        "明日" | "tomorrow" => Some(today + chrono::Duration::days(1)),
        "明後日" => Some(today + chrono::Duration::days(2)),
        "来週" | "next week" => Some(today + chrono::Duration::days(7)),
        "来月" | "next month" => {
            let (y, m) = if today.month() == 12 {
                (today.year() + 1, 1)
            } else {
                (today.year(), today.month() + 1)
            };
            let d = today.day().min(days_in_month(y, m));
            NaiveDate::from_ymd_opt(y, m, d)
        }
        // 曜日指定 (次のその曜日)
        "月曜" | "月" | "mon" | "monday" => Some(next_weekday(today, chrono::Weekday::Mon)),
        "火曜" | "火" | "tue" | "tuesday" => Some(next_weekday(today, chrono::Weekday::Tue)),
        "水曜" | "水" | "wed" | "wednesday" => Some(next_weekday(today, chrono::Weekday::Wed)),
        "木曜" | "木" | "thu" | "thursday" => Some(next_weekday(today, chrono::Weekday::Thu)),
        "金曜" | "金" | "fri" | "friday" => Some(next_weekday(today, chrono::Weekday::Fri)),
        "土曜" | "土" | "sat" | "saturday" => Some(next_weekday(today, chrono::Weekday::Sat)),
        "日曜" | "日" | "sun" | "sunday" => Some(next_weekday(today, chrono::Weekday::Sun)),
        _ => None,
    }
}

fn next_weekday(from: NaiveDate, target: chrono::Weekday) -> NaiveDate {
    let current = from.weekday().num_days_from_monday();
    let target_num = target.num_days_from_monday();
    let days_ahead = if target_num <= current {
        7 - (current - target_num)
    } else {
        target_num - current
    };
    from + chrono::Duration::days(days_ahead as i64)
}

fn days_in_month(year: i32, month: u32) -> u32 {
    let next = if month == 12 {
        NaiveDate::from_ymd_opt(year + 1, 1, 1)
    } else {
        NaiveDate::from_ymd_opt(year, month + 1, 1)
    };
    next.unwrap().pred_opt().unwrap().day()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_yyyy_mm_dd() {
        let d = parse_fuzzy_date("2026-04-15").unwrap();
        assert_eq!(d, NaiveDate::from_ymd_opt(2026, 4, 15).unwrap());
    }

    #[test]
    fn test_parse_today() {
        let today = Local::now().date_naive();
        assert_eq!(parse_fuzzy_date("今日").unwrap(), today);
        assert_eq!(parse_fuzzy_date("today").unwrap(), today);
    }

    #[test]
    fn test_parse_tomorrow() {
        let tomorrow = Local::now().date_naive() + chrono::Duration::days(1);
        assert_eq!(parse_fuzzy_date("明日").unwrap(), tomorrow);
        assert_eq!(parse_fuzzy_date("tomorrow").unwrap(), tomorrow);
    }

    #[test]
    fn test_parse_day_after_tomorrow() {
        let dat = Local::now().date_naive() + chrono::Duration::days(2);
        assert_eq!(parse_fuzzy_date("明後日").unwrap(), dat);
    }

    #[test]
    fn test_parse_next_week() {
        let nw = Local::now().date_naive() + chrono::Duration::days(7);
        assert_eq!(parse_fuzzy_date("来週").unwrap(), nw);
        assert_eq!(parse_fuzzy_date("next week").unwrap(), nw);
    }

    #[test]
    fn test_parse_next_month() {
        let today = Local::now().date_naive();
        let result = parse_fuzzy_date("来月").unwrap();
        if today.month() == 12 {
            assert_eq!(result.year(), today.year() + 1);
            assert_eq!(result.month(), 1);
        } else {
            assert_eq!(result.month(), today.month() + 1);
        }
    }

    #[test]
    fn test_parse_weekday() {
        let today = Local::now().date_naive();
        let fri = parse_fuzzy_date("金曜").unwrap();
        assert_eq!(fri.weekday(), chrono::Weekday::Fri);
        assert!(fri > today);
        assert!((fri - today).num_days() <= 7);

        let mon = parse_fuzzy_date("mon").unwrap();
        assert_eq!(mon.weekday(), chrono::Weekday::Mon);
        assert!(mon > today);
    }

    #[test]
    fn test_parse_invalid() {
        assert!(parse_fuzzy_date("abc").is_none());
        assert!(parse_fuzzy_date("").is_none());
    }
}
