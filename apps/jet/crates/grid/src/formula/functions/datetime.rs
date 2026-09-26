use crate::core::cell::CellValue;
use crate::core::error::CellError;

// Excel date constants
const EXCEL_EPOCH_YEAR: i32 = 1900;
const EXCEL_LEAP_YEAR_BUG_DAY: i64 = 60; // Feb 29, 1900 (phantom day)

/// Convert a date to Excel serial number (days since January 1, 1900)
/// Excel has a bug where it treats 1900 as a leap year, so we account for that
fn date_to_serial(year: i32, month: u32, day: u32) -> Result<f64, CellError> {
    // Validate inputs
    if !(1900..=9999).contains(&year) {
        return Err(CellError::NumError);
    }
    if !(1..=12).contains(&month) {
        return Err(CellError::NumError);
    }
    if day < 1 || day > days_in_month(year, month) {
        return Err(CellError::NumError);
    }

    // Calculate days from epoch (1900/1/1 = serial 1)
    let mut days = 0i64;

    // Add days for complete years
    for y in EXCEL_EPOCH_YEAR..year {
        days += if is_leap_year(y) { 366 } else { 365 };
    }

    // Add days for complete months in the current year
    for m in 1..month {
        days += days_in_month(year, m) as i64;
    }

    // Add the day
    days += day as i64;

    // Excel's leap year bug: Excel treats 1900 as a leap year (it's not)
    // For dates on or after March 1, 1900, we add 1 to match Excel's serial numbers
    if year > 1900 || (year == 1900 && month > 2) {
        days += 1;
    }

    Ok(days as f64)
}

/// Convert Excel serial number to (year, month, day)
fn serial_to_date(serial: f64) -> Result<(i32, u32, u32), CellError> {
    if !(1.0..2958466.0).contains(&serial) {
        // Valid range: 1900-01-01 to 9999-12-31
        return Err(CellError::NumError);
    }

    let mut days = serial.floor() as i64;

    // Account for Excel's leap year bug (serial 60 is the phantom Feb 29, 1900)
    // For serials > 60, we need to subtract 1 to get the real date
    if days > EXCEL_LEAP_YEAR_BUG_DAY {
        days -= 1;
    } else if days == EXCEL_LEAP_YEAR_BUG_DAY {
        // This is the phantom date Feb 29, 1900
        return Ok((1900, 2, 29));
    }

    // Find the year
    let mut year = EXCEL_EPOCH_YEAR;
    let mut remaining_days = days;

    while remaining_days > 0 {
        let days_in_year = if is_leap_year(year) { 366 } else { 365 };
        if remaining_days <= days_in_year {
            break;
        }
        remaining_days -= days_in_year;
        year += 1;
    }

    // Find the month
    let mut month = 1u32;
    while month <= 12 {
        let dim = days_in_month(year, month) as i64;
        if remaining_days <= dim {
            break;
        }
        remaining_days -= dim;
        month += 1;
    }

    let day = remaining_days as u32;

    if day < 1 || day > days_in_month(year, month) {
        return Err(CellError::NumError);
    }

    Ok((year, month, day))
}

/// Check if a year is a leap year
fn is_leap_year(year: i32) -> bool {
    (year % 4 == 0 && year % 100 != 0) || (year % 400 == 0)
}

/// Get the number of days in a month
fn days_in_month(year: i32, month: u32) -> u32 {
    match month {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        2 => {
            if is_leap_year(year) {
                29
            } else {
                28
            }
        }
        _ => 0,
    }
}

/// TODAY - Returns current date as serial number (no time component)
pub fn today(values: &[CellValue]) -> CellValue {
    if !values.is_empty() {
        return CellValue::Error(CellError::InvalidValue);
    }

    // Get current date in UTC
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap();
    let days_since_epoch = now.as_secs() / 86400;

    // Unix epoch is Jan 1, 1970 = Excel serial 25569
    let excel_serial = 25569.0 + days_since_epoch as f64;

    CellValue::Number(excel_serial)
}

/// NOW - Returns current date and time as serial number
pub fn now(values: &[CellValue]) -> CellValue {
    if !values.is_empty() {
        return CellValue::Error(CellError::InvalidValue);
    }

    // Get current datetime in UTC
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap();

    // Unix epoch is Jan 1, 1970 = Excel serial 25569
    let excel_serial = 25569.0 + now.as_secs() as f64 / 86400.0;

    CellValue::Number(excel_serial)
}

/// DATE - Creates a date serial from year, month, day
pub fn date(values: &[CellValue]) -> CellValue {
    if values.len() != 3 {
        return CellValue::Error(CellError::InvalidValue);
    }

    // Check for errors in arguments
    for value in values {
        if let CellValue::Error(e) = value {
            return CellValue::Error(e.clone());
        }
    }

    let year = match values[0].as_number() {
        Some(n) => n.floor() as i32,
        None => return CellValue::Error(CellError::InvalidValue),
    };

    let month = match values[1].as_number() {
        Some(n) => n.floor() as u32,
        None => return CellValue::Error(CellError::InvalidValue),
    };

    let day = match values[2].as_number() {
        Some(n) => n.floor() as u32,
        None => return CellValue::Error(CellError::InvalidValue),
    };

    match date_to_serial(year, month, day) {
        Ok(serial) => CellValue::Number(serial),
        Err(e) => CellValue::Error(e),
    }
}

/// TIME - Returns time as fraction of day
pub fn time(values: &[CellValue]) -> CellValue {
    if values.len() != 3 {
        return CellValue::Error(CellError::InvalidValue);
    }

    // Check for errors in arguments
    for value in values {
        if let CellValue::Error(e) = value {
            return CellValue::Error(e.clone());
        }
    }

    let hour = match values[0].as_number() {
        Some(n) => n.floor() as i32,
        None => return CellValue::Error(CellError::InvalidValue),
    };

    let minute = match values[1].as_number() {
        Some(n) => n.floor() as i32,
        None => return CellValue::Error(CellError::InvalidValue),
    };

    let second = match values[2].as_number() {
        Some(n) => n.floor() as i32,
        None => return CellValue::Error(CellError::InvalidValue),
    };

    if hour < 0 || minute < 0 || second < 0 {
        return CellValue::Error(CellError::NumError);
    }

    // Calculate total seconds
    let total_seconds = hour * 3600 + minute * 60 + second;

    // Convert to fraction of a day (86400 seconds in a day)
    let fraction = (total_seconds % 86400) as f64 / 86400.0;

    CellValue::Number(fraction)
}

/// YEAR - Extracts year from date serial
pub fn year(values: &[CellValue]) -> CellValue {
    if values.len() != 1 {
        return CellValue::Error(CellError::InvalidValue);
    }

    if let CellValue::Error(e) = &values[0] {
        return CellValue::Error(e.clone());
    }

    let serial = match values[0].as_number() {
        Some(n) => n,
        None => return CellValue::Error(CellError::InvalidValue),
    };

    match serial_to_date(serial) {
        Ok((year, _, _)) => CellValue::Number(year as f64),
        Err(e) => CellValue::Error(e),
    }
}

/// MONTH - Extracts month from date serial
pub fn month(values: &[CellValue]) -> CellValue {
    if values.len() != 1 {
        return CellValue::Error(CellError::InvalidValue);
    }

    if let CellValue::Error(e) = &values[0] {
        return CellValue::Error(e.clone());
    }

    let serial = match values[0].as_number() {
        Some(n) => n,
        None => return CellValue::Error(CellError::InvalidValue),
    };

    match serial_to_date(serial) {
        Ok((_, month, _)) => CellValue::Number(month as f64),
        Err(e) => CellValue::Error(e),
    }
}

/// DAY - Extracts day from date serial
pub fn day(values: &[CellValue]) -> CellValue {
    if values.len() != 1 {
        return CellValue::Error(CellError::InvalidValue);
    }

    if let CellValue::Error(e) = &values[0] {
        return CellValue::Error(e.clone());
    }

    let serial = match values[0].as_number() {
        Some(n) => n,
        None => return CellValue::Error(CellError::InvalidValue),
    };

    match serial_to_date(serial) {
        Ok((_, _, day)) => CellValue::Number(day as f64),
        Err(e) => CellValue::Error(e),
    }
}

/// HOUR - Extracts hour from time serial
pub fn hour(values: &[CellValue]) -> CellValue {
    if values.len() != 1 {
        return CellValue::Error(CellError::InvalidValue);
    }

    if let CellValue::Error(e) = &values[0] {
        return CellValue::Error(e.clone());
    }

    let serial = match values[0].as_number() {
        Some(n) => n,
        None => return CellValue::Error(CellError::InvalidValue),
    };

    if serial < 0.0 {
        return CellValue::Error(CellError::NumError);
    }

    // Get fractional part (time component)
    let time_fraction = serial - serial.floor();
    let total_seconds = (time_fraction * 86400.0).round() as i32;
    let hours = (total_seconds / 3600) % 24;

    CellValue::Number(hours as f64)
}

/// MINUTE - Extracts minute from time serial
pub fn minute(values: &[CellValue]) -> CellValue {
    if values.len() != 1 {
        return CellValue::Error(CellError::InvalidValue);
    }

    if let CellValue::Error(e) = &values[0] {
        return CellValue::Error(e.clone());
    }

    let serial = match values[0].as_number() {
        Some(n) => n,
        None => return CellValue::Error(CellError::InvalidValue),
    };

    if serial < 0.0 {
        return CellValue::Error(CellError::NumError);
    }

    // Get fractional part (time component)
    let time_fraction = serial - serial.floor();
    let total_seconds = (time_fraction * 86400.0).round() as i32;
    let minutes = (total_seconds / 60) % 60;

    CellValue::Number(minutes as f64)
}

/// SECOND - Extracts second from time serial
pub fn second(values: &[CellValue]) -> CellValue {
    if values.len() != 1 {
        return CellValue::Error(CellError::InvalidValue);
    }

    if let CellValue::Error(e) = &values[0] {
        return CellValue::Error(e.clone());
    }

    let serial = match values[0].as_number() {
        Some(n) => n,
        None => return CellValue::Error(CellError::InvalidValue),
    };

    if serial < 0.0 {
        return CellValue::Error(CellError::NumError);
    }

    // Get fractional part (time component)
    let time_fraction = serial - serial.floor();
    let total_seconds = (time_fraction * 86400.0).round() as i32;
    let seconds = total_seconds % 60;

    CellValue::Number(seconds as f64)
}

/// DATEDIF - Calculates difference between dates
/// unit: "Y" = years, "M" = months, "D" = days
pub fn datedif(values: &[CellValue]) -> CellValue {
    if values.len() != 3 {
        return CellValue::Error(CellError::InvalidValue);
    }

    // Check for errors in arguments
    for value in values {
        if let CellValue::Error(e) = value {
            return CellValue::Error(e.clone());
        }
    }

    let start_serial = match values[0].as_number() {
        Some(n) => n,
        None => return CellValue::Error(CellError::InvalidValue),
    };

    let end_serial = match values[1].as_number() {
        Some(n) => n,
        None => return CellValue::Error(CellError::InvalidValue),
    };

    let unit = match &values[2] {
        CellValue::Text(s) => s.to_uppercase(),
        _ => return CellValue::Error(CellError::InvalidValue),
    };

    if start_serial > end_serial {
        return CellValue::Error(CellError::NumError);
    }

    let (start_year, start_month, start_day) = match serial_to_date(start_serial) {
        Ok(date) => date,
        Err(e) => return CellValue::Error(e),
    };

    let (end_year, end_month, end_day) = match serial_to_date(end_serial) {
        Ok(date) => date,
        Err(e) => return CellValue::Error(e),
    };

    match unit.as_str() {
        "D" => {
            // Days difference
            let days = (end_serial.floor() - start_serial.floor()) as i32;
            CellValue::Number(days as f64)
        }
        "M" => {
            // Months difference
            let mut months = (end_year - start_year) * 12 + (end_month as i32 - start_month as i32);
            if end_day < start_day {
                months -= 1;
            }
            CellValue::Number(months as f64)
        }
        "Y" => {
            // Years difference
            let mut years = end_year - start_year;
            if end_month < start_month || (end_month == start_month && end_day < start_day) {
                years -= 1;
            }
            CellValue::Number(years as f64)
        }
        _ => CellValue::Error(CellError::InvalidValue),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_date_to_serial() {
        // Test January 1, 1900 (day 1)
        assert_eq!(date_to_serial(1900, 1, 1).unwrap(), 1.0);

        // Test January 2, 1900 (day 2)
        assert_eq!(date_to_serial(1900, 1, 2).unwrap(), 2.0);

        // Test Feb 28, 1900 (day 59, before leap year bug)
        assert_eq!(date_to_serial(1900, 2, 28).unwrap(), 59.0);

        // Test March 1, 1900 (day 61, after leap year bug adjustment)
        assert_eq!(date_to_serial(1900, 3, 1).unwrap(), 61.0);

        // Test a known date: January 1, 2000
        assert_eq!(date_to_serial(2000, 1, 1).unwrap(), 36526.0);

        // Test June 15, 2024
        // Calculated: 45290 days (1900-2023) + 167 days (Jan-Jun 15 in 2024) + 1 (Excel bug) = 45458
        assert_eq!(date_to_serial(2024, 6, 15).unwrap(), 45458.0);
    }

    #[test]
    fn test_serial_to_date() {
        // Test day 1
        assert_eq!(serial_to_date(1.0).unwrap(), (1900, 1, 1));

        // Test day 2
        assert_eq!(serial_to_date(2.0).unwrap(), (1900, 1, 2));

        // Test day 59 (Feb 28, 1900)
        assert_eq!(serial_to_date(59.0).unwrap(), (1900, 2, 28));

        // Test day 60 (Feb 29, 1900 - phantom date)
        assert_eq!(serial_to_date(60.0).unwrap(), (1900, 2, 29));

        // Test day 61 (March 1, 1900, after leap year bug)
        assert_eq!(serial_to_date(61.0).unwrap(), (1900, 3, 1));

        // Test January 1, 2000
        assert_eq!(serial_to_date(36526.0).unwrap(), (2000, 1, 1));

        // Test June 15, 2024 (serial 45458)
        assert_eq!(serial_to_date(45458.0).unwrap(), (2024, 6, 15));
    }

    #[test]
    fn test_date_function() {
        let result = date(&[
            CellValue::Number(2024.0),
            CellValue::Number(6.0),
            CellValue::Number(15.0),
        ]);
        assert_eq!(result, CellValue::Number(45458.0));

        // Test error cases
        assert!(matches!(
            date(&[
                CellValue::Number(1899.0),
                CellValue::Number(1.0),
                CellValue::Number(1.0)
            ]),
            CellValue::Error(CellError::NumError)
        ));

        // Test wrong number of arguments
        assert!(matches!(
            date(&[CellValue::Number(2024.0), CellValue::Number(6.0)]),
            CellValue::Error(CellError::InvalidValue)
        ));
    }

    #[test]
    fn test_time_function() {
        // Test noon (12:00:00) = 0.5
        let result = time(&[
            CellValue::Number(12.0),
            CellValue::Number(0.0),
            CellValue::Number(0.0),
        ]);
        assert_eq!(result, CellValue::Number(0.5));

        // Test 6 AM (06:00:00) = 0.25
        let result = time(&[
            CellValue::Number(6.0),
            CellValue::Number(0.0),
            CellValue::Number(0.0),
        ]);
        assert_eq!(result, CellValue::Number(0.25));

        // Test 18:30:00 (6:30 PM)
        let result = time(&[
            CellValue::Number(18.0),
            CellValue::Number(30.0),
            CellValue::Number(0.0),
        ]);
        let expected = (18.0 * 3600.0 + 30.0 * 60.0) / 86400.0;
        assert_eq!(result, CellValue::Number(expected));

        // Test negative hour
        assert!(matches!(
            time(&[
                CellValue::Number(-1.0),
                CellValue::Number(0.0),
                CellValue::Number(0.0)
            ]),
            CellValue::Error(CellError::NumError)
        ));
    }

    #[test]
    fn test_year_month_day() {
        // Test June 15, 2024 (serial 45458)
        let serial = CellValue::Number(45458.0);

        assert_eq!(year(&[serial.clone()]), CellValue::Number(2024.0));
        assert_eq!(month(&[serial.clone()]), CellValue::Number(6.0));
        assert_eq!(day(&[serial]), CellValue::Number(15.0));

        // Test January 1, 1900
        let serial = CellValue::Number(1.0);
        assert_eq!(year(&[serial.clone()]), CellValue::Number(1900.0));
        assert_eq!(month(&[serial.clone()]), CellValue::Number(1.0));
        assert_eq!(day(&[serial]), CellValue::Number(1.0));
    }

    #[test]
    fn test_hour_minute_second() {
        // Test 18:30:45 as fraction
        let time_fraction = (18.0 * 3600.0 + 30.0 * 60.0 + 45.0) / 86400.0;
        let serial = CellValue::Number(time_fraction);

        assert_eq!(hour(&[serial.clone()]), CellValue::Number(18.0));
        assert_eq!(minute(&[serial.clone()]), CellValue::Number(30.0));
        assert_eq!(second(&[serial]), CellValue::Number(45.0));

        // Test full datetime (date + time)
        let datetime = 45467.0 + time_fraction; // June 15, 2024 18:30:45
        let serial = CellValue::Number(datetime);

        assert_eq!(hour(&[serial.clone()]), CellValue::Number(18.0));
        assert_eq!(minute(&[serial.clone()]), CellValue::Number(30.0));
        assert_eq!(second(&[serial]), CellValue::Number(45.0));
    }

    #[test]
    fn test_datedif_days() {
        // 10 days difference
        let start = CellValue::Number(date_to_serial(2024, 1, 1).unwrap());
        let end = CellValue::Number(date_to_serial(2024, 1, 11).unwrap());
        let unit = CellValue::Text("D".to_string());

        assert_eq!(datedif(&[start, end, unit]), CellValue::Number(10.0));
    }

    #[test]
    fn test_datedif_months() {
        // Exactly 3 months
        let start = CellValue::Number(date_to_serial(2024, 1, 15).unwrap());
        let end = CellValue::Number(date_to_serial(2024, 4, 15).unwrap());
        let unit = CellValue::Text("M".to_string());

        assert_eq!(datedif(&[start, end, unit]), CellValue::Number(3.0));

        // 2 months (not quite 3)
        let start = CellValue::Number(date_to_serial(2024, 1, 15).unwrap());
        let end = CellValue::Number(date_to_serial(2024, 4, 14).unwrap());
        let unit = CellValue::Text("M".to_string());

        assert_eq!(datedif(&[start, end, unit]), CellValue::Number(2.0));
    }

    #[test]
    fn test_datedif_years() {
        // Exactly 2 years
        let start = CellValue::Number(date_to_serial(2022, 6, 15).unwrap());
        let end = CellValue::Number(date_to_serial(2024, 6, 15).unwrap());
        let unit = CellValue::Text("Y".to_string());

        assert_eq!(datedif(&[start, end, unit]), CellValue::Number(2.0));

        // Not quite 2 years
        let start = CellValue::Number(date_to_serial(2022, 6, 15).unwrap());
        let end = CellValue::Number(date_to_serial(2024, 6, 14).unwrap());
        let unit = CellValue::Text("Y".to_string());

        assert_eq!(datedif(&[start, end, unit]), CellValue::Number(1.0));
    }

    #[test]
    fn test_datedif_errors() {
        // Start date after end date
        let start = CellValue::Number(date_to_serial(2024, 6, 15).unwrap());
        let end = CellValue::Number(date_to_serial(2024, 1, 1).unwrap());
        let unit = CellValue::Text("D".to_string());

        assert!(matches!(
            datedif(&[start, end, unit]),
            CellValue::Error(CellError::NumError)
        ));

        // Invalid unit
        let start = CellValue::Number(date_to_serial(2024, 1, 1).unwrap());
        let end = CellValue::Number(date_to_serial(2024, 6, 15).unwrap());
        let unit = CellValue::Text("X".to_string());

        assert!(matches!(
            datedif(&[start, end, unit]),
            CellValue::Error(CellError::InvalidValue)
        ));
    }

    #[test]
    fn test_today_now() {
        // TODAY should return no fractional part
        let today_result = today(&[]);
        if let CellValue::Number(n) = today_result {
            assert_eq!(n.fract(), 0.0);
        } else {
            panic!("TODAY should return a number");
        }

        // NOW should return a number (may have fractional part)
        let now_result = now(&[]);
        assert!(matches!(now_result, CellValue::Number(_)));

        // Both should reject arguments
        assert!(matches!(
            today(&[CellValue::Number(1.0)]),
            CellValue::Error(CellError::InvalidValue)
        ));
        assert!(matches!(
            now(&[CellValue::Number(1.0)]),
            CellValue::Error(CellError::InvalidValue)
        ));
    }

    #[test]
    fn test_error_propagation() {
        let error = CellValue::Error(CellError::DivisionByZero);

        assert!(matches!(
            date(&[
                error.clone(),
                CellValue::Number(1.0),
                CellValue::Number(1.0)
            ]),
            CellValue::Error(CellError::DivisionByZero)
        ));

        assert!(matches!(
            year(&[error.clone()]),
            CellValue::Error(CellError::DivisionByZero)
        ));

        assert!(matches!(
            datedif(&[
                error.clone(),
                CellValue::Number(1.0),
                CellValue::Text("D".to_string())
            ]),
            CellValue::Error(CellError::DivisionByZero)
        ));
    }

    #[test]
    fn test_leap_year() {
        // 2000 is a leap year (divisible by 400)
        assert!(is_leap_year(2000));

        // 2024 is a leap year (divisible by 4, not by 100)
        assert!(is_leap_year(2024));

        // 1900 is NOT a leap year (divisible by 100 but not 400)
        assert!(!is_leap_year(1900));

        // 2023 is not a leap year
        assert!(!is_leap_year(2023));
    }

    #[test]
    fn test_days_in_month() {
        // January has 31 days
        assert_eq!(days_in_month(2024, 1), 31);

        // February in leap year has 29 days
        assert_eq!(days_in_month(2024, 2), 29);

        // February in non-leap year has 28 days
        assert_eq!(days_in_month(2023, 2), 28);

        // April has 30 days
        assert_eq!(days_in_month(2024, 4), 30);
    }
}
