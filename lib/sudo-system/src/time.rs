use std::{ops::{Add, Sub}, mem::MaybeUninit, io::{Write, Read}};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct SystemTime {
    secs: i64,
    nsecs: i64,
}

impl SystemTime {
    pub(crate) fn new(secs: i64, nsecs: i64) -> SystemTime {
        let mut secs = secs + nsecs / 1_000_000_000;
        let mut nsecs = nsecs % 1_000_000_000;

        if secs > 0 && nsecs < 0 {
            secs -= 1;
            nsecs += 1_000_000_000;
        } else if secs < 0 && nsecs > 0 {
            secs += 1;
            nsecs -= 1_000_000_000;
        }

        SystemTime {
            secs,
            nsecs,
        }
    }

    pub fn now() -> std::io::Result<SystemTime> {
        let mut spec = MaybeUninit::<libc::timespec>::uninit();
        sudo_cutils::cerr(unsafe { libc::clock_gettime(libc::CLOCK_BOOTTIME, spec.as_mut_ptr() )})?;
        let spec = unsafe { spec.assume_init() };
        Ok(spec.into())
    }

    pub(crate) fn encode(&self, target: &mut impl Write) -> std::io::Result<()> {
        let secs = self.secs.to_ne_bytes();
        let nsecs = self.nsecs.to_ne_bytes();
        target.write_all(&secs)?;
        target.write_all(&nsecs)?;
        Ok(())
    }

    pub(crate) fn decode(from: &mut impl Read) -> std::io::Result<SystemTime> {
        let mut sec_bytes = [0; 8];
        let mut nsec_bytes = [0; 8];

        from.read_exact(&mut sec_bytes)?;
        from.read_exact(&mut nsec_bytes)?;

        Ok(SystemTime::new(i64::from_ne_bytes(sec_bytes), i64::from_ne_bytes(nsec_bytes)))
    }
}

impl Sub<SystemTime> for SystemTime {
    type Output = Duration;

    fn sub(self, rhs: SystemTime) -> Self::Output {
        Duration::new(self.secs - rhs.secs, self.nsecs - rhs.nsecs)
    }
}

impl Add<Duration> for SystemTime {
    type Output = SystemTime;

    fn add(self, rhs: Duration) -> Self::Output {
        SystemTime::new(self.secs + rhs.secs, self.nsecs + rhs.nsecs)
    }
}

impl Sub<Duration> for SystemTime {
    type Output = SystemTime;

    fn sub(self, rhs: Duration) -> Self::Output {
        SystemTime::new(self.secs - rhs.secs, self.nsecs - rhs.nsecs)
    }
}

#[derive(Debug, PartialEq, Eq, PartialOrd, Ord, Clone, Copy)]
pub struct Duration {
    secs: i64,
    nsecs: i64,
}

impl Duration {
    pub fn new(secs: i64, nsecs: i64) -> Duration {
        let mut secs = secs + nsecs / 1_000_000_000;
        let mut nsecs = nsecs % 1_000_000_000;

        if secs > 0 && nsecs < 0 {
            secs -= 1;
            nsecs += 1_000_000_000;
        } else if secs < 0 && nsecs > 0 {
            secs += 1;
            nsecs -= 1_000_000_000;
        }

        Duration {
            secs,
            nsecs,
        }
    }

    pub fn seconds(secs: i64) -> Duration {
        Duration::new(secs, 0)
    }

    pub fn minutes(minutes: i64) -> Duration {
        Duration::seconds(minutes * 60)
    }

    pub fn milliseconds(ms: i64) -> Duration {
        let secs = ms / 1000;
        let ms = ms % 1000;
        Duration::new(secs, ms * 1_000_000)
    }
}

impl Add<Duration> for Duration {
    type Output = Duration;

    fn add(self, rhs: Duration) -> Self::Output {
        Duration::new(self.secs + rhs.secs, self.nsecs + rhs.nsecs)
    }
}

impl Sub<Duration> for Duration {
    type Output = Duration;

    fn sub(self, rhs: Duration) -> Self::Output {
        Duration::new(self.secs - rhs.secs, self.nsecs - rhs.nsecs)
    }
}

impl From<libc::timespec> for SystemTime {
    fn from(value: libc::timespec) -> Self {
        SystemTime::new(value.tv_sec as _, value.tv_nsec as _)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_new_durations_and_times() {
        assert_eq!(Duration::new(1, 1_000_000_000), Duration::seconds(2));
        assert_eq!(Duration::new(-2, 500_000_000), Duration::seconds(-1) + Duration::milliseconds(-500));

        assert_eq!(SystemTime::new(-1, 2_000_000_000), SystemTime::new(1, 0));
        assert_eq!(SystemTime::new(2, -500_000_000), SystemTime::new(1, 500_000_000));
    }

    #[test]
    fn test_time_ops() {
        assert_eq!(Duration::seconds(2) + Duration::seconds(3), Duration::seconds(5));
        assert_eq!(Duration::seconds(3) - Duration::seconds(1), Duration::seconds(2));
        assert_eq!(Duration::seconds(-10) + Duration::seconds(-5), Duration::seconds(-15));
        assert_eq!(Duration::milliseconds(5555) + Duration::milliseconds(5555), Duration::seconds(11) + Duration::milliseconds(110));
        assert_eq!(Duration::seconds(10) - Duration::seconds(-5), Duration::seconds(15));

        assert_eq!(SystemTime::new(0, 0) + Duration::seconds(3), SystemTime::new(3, 0));
        assert_eq!(SystemTime::new(10, 0) - Duration::seconds(4), SystemTime::new(6, 0));
    }
}
