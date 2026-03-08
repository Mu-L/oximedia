//! Retry policies and strategies

use std::time::Duration;

/// Retry policy configuration
#[derive(Debug, Clone)]
pub struct RetryPolicy {
    pub max_attempts: u32,
    pub strategy: RetryStrategy,
}

impl Default for RetryPolicy {
    fn default() -> Self {
        Self {
            max_attempts: 3,
            strategy: RetryStrategy::ExponentialBackoff {
                initial_delay: Duration::from_secs(1),
                max_delay: Duration::from_secs(60),
                multiplier: 2.0,
            },
        }
    }
}

impl RetryPolicy {
    /// Create a new retry policy
    #[must_use]
    pub fn new(max_attempts: u32, strategy: RetryStrategy) -> Self {
        Self {
            max_attempts,
            strategy,
        }
    }

    /// Calculate the delay for a given attempt
    #[must_use]
    pub fn calculate_delay(&self, attempt: u32) -> Duration {
        self.strategy.calculate_delay(attempt)
    }
}

/// Retry strategies
#[derive(Debug, Clone)]
pub enum RetryStrategy {
    /// Fixed delay between retries
    Fixed(Duration),

    /// Linear backoff with increasing delay
    LinearBackoff {
        initial_delay: Duration,
        increment: Duration,
    },

    /// Exponential backoff with jitter
    ExponentialBackoff {
        initial_delay: Duration,
        max_delay: Duration,
        multiplier: f64,
    },

    /// Custom delay function
    Custom(Vec<Duration>),
}

impl RetryStrategy {
    /// Calculate the delay for a given attempt
    #[must_use]
    pub fn calculate_delay(&self, attempt: u32) -> Duration {
        match self {
            Self::Fixed(delay) => *delay,

            Self::LinearBackoff {
                initial_delay,
                increment,
            } => {
                let total_increments = attempt.saturating_sub(1);
                *initial_delay + *increment * total_increments
            }

            Self::ExponentialBackoff {
                initial_delay,
                max_delay,
                multiplier,
            } => {
                let delay_ms = initial_delay.as_millis() as f64
                    * multiplier.powi(attempt.saturating_sub(1) as i32);

                let delay = Duration::from_millis(delay_ms.min(u64::MAX as f64) as u64);

                delay.min(*max_delay)
            }

            Self::Custom(delays) => {
                let index = (attempt.saturating_sub(1) as usize).min(delays.len() - 1);
                delays[index]
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_fixed_delay() {
        let strategy = RetryStrategy::Fixed(Duration::from_secs(5));

        assert_eq!(strategy.calculate_delay(1), Duration::from_secs(5));
        assert_eq!(strategy.calculate_delay(2), Duration::from_secs(5));
        assert_eq!(strategy.calculate_delay(3), Duration::from_secs(5));
    }

    #[test]
    fn test_linear_backoff() {
        let strategy = RetryStrategy::LinearBackoff {
            initial_delay: Duration::from_secs(1),
            increment: Duration::from_secs(2),
        };

        assert_eq!(strategy.calculate_delay(1), Duration::from_secs(1));
        assert_eq!(strategy.calculate_delay(2), Duration::from_secs(3));
        assert_eq!(strategy.calculate_delay(3), Duration::from_secs(5));
    }

    #[test]
    fn test_exponential_backoff() {
        let strategy = RetryStrategy::ExponentialBackoff {
            initial_delay: Duration::from_secs(1),
            max_delay: Duration::from_secs(60),
            multiplier: 2.0,
        };

        assert_eq!(strategy.calculate_delay(1), Duration::from_secs(1));
        assert_eq!(strategy.calculate_delay(2), Duration::from_secs(2));
        assert_eq!(strategy.calculate_delay(3), Duration::from_secs(4));
        assert_eq!(strategy.calculate_delay(4), Duration::from_secs(8));
    }

    #[test]
    fn test_exponential_backoff_max_delay() {
        let strategy = RetryStrategy::ExponentialBackoff {
            initial_delay: Duration::from_secs(1),
            max_delay: Duration::from_secs(10),
            multiplier: 2.0,
        };

        // Should cap at max_delay
        assert_eq!(strategy.calculate_delay(10), Duration::from_secs(10));
    }

    #[test]
    fn test_custom_delays() {
        let strategy = RetryStrategy::Custom(vec![
            Duration::from_secs(1),
            Duration::from_secs(5),
            Duration::from_secs(10),
        ]);

        assert_eq!(strategy.calculate_delay(1), Duration::from_secs(1));
        assert_eq!(strategy.calculate_delay(2), Duration::from_secs(5));
        assert_eq!(strategy.calculate_delay(3), Duration::from_secs(10));
        // Should use last delay for attempts beyond the list
        assert_eq!(strategy.calculate_delay(4), Duration::from_secs(10));
    }

    #[test]
    fn test_default_policy() {
        let policy = RetryPolicy::default();
        assert_eq!(policy.max_attempts, 3);
    }

    #[test]
    fn test_policy_calculate_delay() {
        let policy = RetryPolicy::new(3, RetryStrategy::Fixed(Duration::from_secs(5)));

        assert_eq!(policy.calculate_delay(1), Duration::from_secs(5));
        assert_eq!(policy.calculate_delay(2), Duration::from_secs(5));
    }
}
