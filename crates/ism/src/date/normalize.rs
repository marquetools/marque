// SPDX-FileCopyrightText: 2026 Knitli Inc.
//
// SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0

use std::fmt;

use super::{ApproxIsmDate, ApproxQualifier, IsmDate, UtcOffset};

impl fmt::Display for UtcOffset {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.minutes == 0 {
            write!(f, "Z")
        } else {
            let sign = if self.minutes >= 0 { '+' } else { '-' };
            let abs = self.minutes.unsigned_abs();
            write!(f, "{}{:02}:{:02}", sign, abs / 60, abs % 60)
        }
    }
}

impl fmt::Display for IsmDate {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            IsmDate::Year(y) => write!(f, "{:04}", y),
            IsmDate::YearMonth(y, m) => write!(f, "{:04}-{:02}", y, m),
            IsmDate::Date(y, m, d) => write!(f, "{:04}-{:02}-{:02}", y, m, d),
            IsmDate::DateHourMin {
                year,
                month,
                day,
                hour,
                minute,
                offset,
            } => {
                write!(
                    f,
                    "{:04}-{:02}-{:02}T{:02}:{:02}",
                    year, month, day, hour, minute
                )?;
                if let Some(o) = offset {
                    write!(f, "{o}")?;
                }
                Ok(())
            }
            IsmDate::DateTime {
                year,
                month,
                day,
                hour,
                minute,
                second,
                nanosecond,
                offset,
            } => {
                write!(
                    f,
                    "{:04}-{:02}-{:02}T{:02}:{:02}:{:02}",
                    year, month, day, hour, minute, second
                )?;
                if *nanosecond > 0 {
                    let s = format!("{:09}", nanosecond);
                    let trimmed = s.trim_end_matches('0');
                    write!(f, ".{trimmed}")?;
                }
                if let Some(o) = offset {
                    write!(f, "{o}")?;
                }
                Ok(())
            }
        }
    }
}

impl fmt::Display for ApproxQualifier {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let s = match self {
            ApproxQualifier::FirstQtr => "1st qtr",
            ApproxQualifier::SecondQtr => "2nd qtr",
            ApproxQualifier::ThirdQtr => "3rd qtr",
            ApproxQualifier::FourthQtr => "4th qtr",
            ApproxQualifier::Circa => "circa",
            ApproxQualifier::Early => "early",
            ApproxQualifier::Mid => "mid",
            ApproxQualifier::Late => "late",
        };
        f.write_str(s)
    }
}

impl fmt::Display for ApproxIsmDate {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if let Some(q) = self.qualifier {
            write!(f, "{} {}", q, self.date)
        } else {
            write!(f, "{}", self.date)
        }
    }
}
