// Adapted from code originally in [CocoIndex](https://CocoIndex)
// Original code from CocoIndex is copyrighted by CocoIndex
// and licensed under the Apache-2.0 License.
// SPDX-License-Identifier: Apache-2.0
// SPDX-FileCopyrightText: 2026 CocoIndex
//
// All modifications from the upstream for Marque are copyrighted by Knitli Inc.
// SPDX-FileCopyrightText: 2026 Knitli Inc. (Marque)
// SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0

use std::sync::Arc;
use tokio::sync::{AcquireError, OwnedSemaphorePermit, Semaphore};

struct WeightedSemaphore {
    downscale_factor: u8,
    downscaled_quota: u32,
    sem: Arc<Semaphore>,
}

impl WeightedSemaphore {
    pub fn new(quota: usize) -> Self {
        let mut downscale_factor = 0;
        let mut downscaled_quota = quota;
        while downscaled_quota > u32::MAX as usize {
            downscaled_quota >>= 1;
            downscale_factor += 1;
        }
        let sem = Arc::new(Semaphore::new(downscaled_quota));
        Self {
            downscaled_quota: downscaled_quota as u32,
            downscale_factor,
            sem,
        }
    }

    async fn acquire_reservation(&self) -> Result<OwnedSemaphorePermit, AcquireError> {
        self.sem.clone().acquire_owned().await
    }

    async fn acquire(
        &self,
        weight: usize,
        reserved: bool,
    ) -> Result<Option<OwnedSemaphorePermit>, AcquireError> {
        let downscaled_weight = (weight >> self.downscale_factor) as u32;
        let capped_weight = downscaled_weight.min(self.downscaled_quota);
        let reserved_weight = if reserved { 1 } else { 0 };
        if reserved_weight >= capped_weight {
            return Ok(None);
        }
        Ok(Some(
            self.sem
                .clone()
                .acquire_many_owned(capped_weight - reserved_weight)
                .await?,
        ))
    }
}

pub struct Options {
    pub max_inflight_rows: Option<usize>,
    pub max_inflight_bytes: Option<usize>,
}

pub struct ConcurrencyControllerPermit {
    _inflight_count_permit: Option<OwnedSemaphorePermit>,
    _inflight_bytes_permit: Option<OwnedSemaphorePermit>,
}

pub struct ConcurrencyController {
    inflight_count_sem: Option<Arc<Semaphore>>,
    inflight_bytes_sem: Option<WeightedSemaphore>,
}

pub static BYTES_UNKNOWN_YET: Option<fn() -> usize> = None;

impl ConcurrencyController {
    pub fn new(exec_options: &Options) -> Self {
        Self {
            inflight_count_sem: exec_options
                .max_inflight_rows
                .map(|max| Arc::new(Semaphore::new(max))),
            inflight_bytes_sem: exec_options.max_inflight_bytes.map(WeightedSemaphore::new),
        }
    }

    /// If `bytes_fn` is `None`, it means the number of bytes is not known yet.
    /// The controller will reserve a minimum number of bytes.
    /// The caller should call `acquire_bytes_with_reservation` with the actual number of bytes later.
    pub async fn acquire(
        &self,
        bytes_fn: Option<impl FnOnce() -> usize>,
    ) -> Result<ConcurrencyControllerPermit, AcquireError> {
        let inflight_count_permit = if let Some(sem) = &self.inflight_count_sem {
            Some(sem.clone().acquire_owned().await?)
        } else {
            None
        };
        let inflight_bytes_permit = if let Some(sem) = &self.inflight_bytes_sem {
            if let Some(bytes_fn) = bytes_fn {
                sem.acquire(bytes_fn(), false).await?
            } else {
                Some(sem.acquire_reservation().await?)
            }
        } else {
            None
        };
        Ok(ConcurrencyControllerPermit {
            _inflight_count_permit: inflight_count_permit,
            _inflight_bytes_permit: inflight_bytes_permit,
        })
    }

    pub async fn acquire_bytes_with_reservation(
        &self,
        bytes_fn: impl FnOnce() -> usize,
    ) -> Result<Option<OwnedSemaphorePermit>, AcquireError> {
        if let Some(sem) = &self.inflight_bytes_sem {
            sem.acquire(bytes_fn(), true).await
        } else {
            Ok(None)
        }
    }
}

pub struct CombinedConcurrencyControllerPermit {
    _permit: ConcurrencyControllerPermit,
    _global_permit: ConcurrencyControllerPermit,
}

pub struct CombinedConcurrencyController {
    controller: ConcurrencyController,
    global_controller: Arc<ConcurrencyController>,
    needs_num_bytes: bool,
}

impl CombinedConcurrencyController {
    pub fn new(exec_options: &Options, global_controller: Arc<ConcurrencyController>) -> Self {
        Self {
            controller: ConcurrencyController::new(exec_options),
            needs_num_bytes: exec_options.max_inflight_bytes.is_some()
                || global_controller.inflight_bytes_sem.is_some(),
            global_controller,
        }
    }

    pub async fn acquire(
        &self,
        bytes_fn: Option<impl FnOnce() -> usize>,
    ) -> Result<CombinedConcurrencyControllerPermit, AcquireError> {
        let num_bytes_fn = if let Some(bytes_fn) = bytes_fn
            && self.needs_num_bytes
        {
            let num_bytes = bytes_fn();
            Some(move || num_bytes)
        } else {
            None
        };

        let permit = self.controller.acquire(num_bytes_fn).await?;
        let global_permit = self.global_controller.acquire(num_bytes_fn).await?;
        Ok(CombinedConcurrencyControllerPermit {
            _permit: permit,
            _global_permit: global_permit,
        })
    }

    pub async fn acquire_bytes_with_reservation(
        &self,
        bytes_fn: impl FnOnce() -> usize,
    ) -> Result<(Option<OwnedSemaphorePermit>, Option<OwnedSemaphorePermit>), AcquireError> {
        let num_bytes = bytes_fn();
        let permit = self
            .controller
            .acquire_bytes_with_reservation(move || num_bytes)
            .await?;
        let global_permit = self
            .global_controller
            .acquire_bytes_with_reservation(move || num_bytes)
            .await?;
        Ok((permit, global_permit))
    }
}

#[cfg(test)]
#[cfg_attr(coverage_nightly, coverage(off))]
mod tests {
    use super::*;
    use std::time::Duration;

    fn unlimited() -> Options {
        Options {
            max_inflight_rows: None,
            max_inflight_bytes: None,
        }
    }

    #[tokio::test]
    async fn acquire_with_no_limits_yields_empty_permit() {
        let cc = ConcurrencyController::new(&unlimited());
        let permit = cc.acquire(Some(|| 100usize)).await.unwrap();
        assert!(permit._inflight_count_permit.is_none());
        assert!(permit._inflight_bytes_permit.is_none());
    }

    #[tokio::test]
    async fn row_limit_blocks_until_permit_released() {
        let cc = Arc::new(ConcurrencyController::new(&Options {
            max_inflight_rows: Some(1),
            max_inflight_bytes: None,
        }));

        let permit = cc.acquire(None::<fn() -> usize>).await.unwrap();

        let cc2 = cc.clone();
        let pending =
            tokio::spawn(async move { cc2.acquire(None::<fn() -> usize>).await.map(|_| ()) });

        // The only row permit is held, so the second acquire cannot complete.
        tokio::time::sleep(Duration::from_millis(20)).await;
        assert!(!pending.is_finished());

        drop(permit);

        tokio::time::timeout(Duration::from_secs(1), pending)
            .await
            .expect("acquire should resolve once the permit is freed")
            .unwrap()
            .unwrap();
    }

    #[tokio::test]
    async fn byte_limit_blocks_until_weight_released() {
        let cc = Arc::new(ConcurrencyController::new(&Options {
            max_inflight_rows: None,
            max_inflight_bytes: Some(10),
        }));

        // Consume the full byte budget.
        let permit = cc.acquire(Some(|| 10usize)).await.unwrap();
        assert!(permit._inflight_bytes_permit.is_some());

        let cc2 = cc.clone();
        let pending = tokio::spawn(async move { cc2.acquire(Some(|| 5usize)).await.map(|_| ()) });

        tokio::time::sleep(Duration::from_millis(20)).await;
        assert!(!pending.is_finished());

        drop(permit);

        tokio::time::timeout(Duration::from_secs(1), pending)
            .await
            .expect("acquire should resolve once bytes are freed")
            .unwrap()
            .unwrap();
    }

    #[tokio::test]
    async fn unknown_byte_count_takes_reservation_then_real_weight() {
        let cc = ConcurrencyController::new(&Options {
            max_inflight_rows: None,
            max_inflight_bytes: Some(100),
        });

        // Byte count unknown yet: a single-permit reservation is taken.
        let permit = cc.acquire(None::<fn() -> usize>).await.unwrap();
        assert!(permit._inflight_bytes_permit.is_some());

        // The actual weight is acquired later, minus the reserved permit.
        let real = cc.acquire_bytes_with_reservation(|| 50usize).await.unwrap();
        assert!(real.is_some());
    }

    #[tokio::test]
    async fn reservation_already_covers_unit_weight() {
        let cc = ConcurrencyController::new(&Options {
            max_inflight_rows: None,
            max_inflight_bytes: Some(100),
        });

        // A weight of 1 is fully covered by the reserved permit, so no extra
        // permit is acquired.
        let extra = cc.acquire_bytes_with_reservation(|| 1usize).await.unwrap();
        assert!(extra.is_none());
    }

    #[tokio::test]
    async fn reservation_without_byte_limit_is_noop() {
        let cc = ConcurrencyController::new(&Options {
            max_inflight_rows: Some(5),
            max_inflight_bytes: None,
        });

        let extra = cc.acquire_bytes_with_reservation(|| 50usize).await.unwrap();
        assert!(extra.is_none());
    }

    #[tokio::test]
    async fn weighted_semaphore_downscales_quota_above_u32_max() {
        let ws = WeightedSemaphore::new((u32::MAX as usize) + 1);
        // One right-shift brings the quota back under u32::MAX.
        assert_eq!(ws.downscale_factor, 1);
        assert_eq!(ws.downscaled_quota, ((u32::MAX as usize + 1) >> 1) as u32);

        // A weight is downscaled by the same factor and can be acquired.
        let permit = ws.acquire((u32::MAX as usize) + 1, false).await.unwrap();
        assert!(permit.is_some());
    }

    #[tokio::test]
    async fn weighted_semaphore_skips_reserved_unit_weight() {
        let ws = WeightedSemaphore::new(100);
        // reserved=true with a capped weight of 1 means the reservation already
        // accounts for the whole weight.
        let permit = ws.acquire(1, true).await.unwrap();
        assert!(permit.is_none());
    }

    #[tokio::test]
    async fn combined_controller_acquires_local_and_global() {
        let global = Arc::new(ConcurrencyController::new(&Options {
            max_inflight_rows: Some(2),
            max_inflight_bytes: Some(100),
        }));
        let combined = CombinedConcurrencyController::new(
            &Options {
                max_inflight_rows: Some(2),
                max_inflight_bytes: Some(100),
            },
            global,
        );

        let permit = combined.acquire(Some(|| 10usize)).await.unwrap();
        drop(permit);
    }

    #[tokio::test]
    async fn combined_controller_acquires_without_byte_fn() {
        // No byte function supplied even though byte limits exist: both layers
        // fall back to the reservation path.
        let global = Arc::new(ConcurrencyController::new(&Options {
            max_inflight_rows: None,
            max_inflight_bytes: Some(100),
        }));
        let combined = CombinedConcurrencyController::new(
            &Options {
                max_inflight_rows: None,
                max_inflight_bytes: Some(100),
            },
            global,
        );

        let permit = combined.acquire(None::<fn() -> usize>).await.unwrap();
        drop(permit);
    }

    #[tokio::test]
    async fn combined_reservation_returns_both_permits() {
        let global = Arc::new(ConcurrencyController::new(&Options {
            max_inflight_rows: None,
            max_inflight_bytes: Some(100),
        }));
        let combined = CombinedConcurrencyController::new(
            &Options {
                max_inflight_rows: None,
                max_inflight_bytes: Some(100),
            },
            global,
        );

        let (local, global) = combined
            .acquire_bytes_with_reservation(|| 50usize)
            .await
            .unwrap();
        assert!(local.is_some());
        assert!(global.is_some());
    }
}
