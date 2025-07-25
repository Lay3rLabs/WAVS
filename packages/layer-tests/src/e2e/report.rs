use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};

#[derive(Clone)]
pub struct TestReport {
    // This is a rough metric for debugging, since it can be interrupted by other async tasks
    timings: Arc<Mutex<TestTimings>>,
}

struct TestTimings {
    per_test_start: HashMap<String, Instant>,
    per_test_duration: HashMap<String, Duration>,
    total_duration: Duration,
    longest_test: Option<(String, Duration)>,
    shortest_test: Option<(String, Duration)>,
    completed_count: HashMap<String, usize>,
}

impl TestTimings {
    pub fn new() -> Self {
        Self {
            per_test_start: HashMap::new(),
            per_test_duration: HashMap::new(),
            total_duration: Duration::ZERO,
            longest_test: None,
            shortest_test: None,
            completed_count: HashMap::new(),
        }
    }

    pub fn start(&mut self, test_name: String) {
        tracing::warn!("*************************************");
        tracing::warn!("Starting test: {}", test_name);
        tracing::warn!("*************************************");
        let start_time = Instant::now();
        if self
            .per_test_start
            .insert(test_name.clone(), start_time)
            .is_some()
        {
            panic!("Test {} was already started!", test_name);
        }
    }

    pub fn end(&mut self, test_name: String) {
        let duration = self.per_test_start.get(&test_name).unwrap().elapsed();

        tracing::warn!("*************************************");
        tracing::warn!(
            "Test {test_name} completed ({}ms start to finish)",
            duration.as_millis()
        );
        tracing::warn!("*************************************");

        let count = self.completed_count.entry(test_name.clone()).or_insert(0);
        *count += 1;

        if *count > 1 {
            // we only want to update durations for the first run
            tracing::warn!("{} test been run {} times in total.", test_name, count);
            return;
        }
        self.per_test_duration.insert(test_name.clone(), duration);
        self.total_duration += duration;

        if let Some((_, longest_duration)) = &self.longest_test {
            if duration > *longest_duration {
                self.longest_test = Some((test_name.clone(), duration));
            }
        } else {
            self.longest_test = Some((test_name.clone(), duration));
        }

        if let Some((_, shortest_duration)) = &self.shortest_test {
            if duration < *shortest_duration {
                self.shortest_test = Some((test_name.clone(), duration));
            }
        } else {
            self.shortest_test = Some((test_name, duration));
        }
    }
}

impl TestReport {
    pub fn new() -> Self {
        Self {
            timings: Arc::new(Mutex::new(TestTimings::new())),
        }
    }

    pub fn start_test(&self, test_name: String) {
        let mut timings = self.timings.lock().unwrap();
        timings.start(test_name);
    }

    pub fn end_test(&self, test_name: String) {
        let mut timings = self.timings.lock().unwrap();
        timings.end(test_name);
    }

    pub fn print(&self) {
        let timings = self.timings.lock().unwrap();
        tracing::warn!("*************************************");
        tracing::warn!("All tests completed!");
        for (test_name, duration) in &timings.per_test_duration {
            tracing::warn!("{} Duration: {:?}", test_name, duration);
        }
        tracing::warn!("Total duration: {:?}", timings.total_duration);
        if let Some((test_name, duration)) = &timings.longest_test {
            tracing::warn!("Longest: {}, Duration: {:?}", test_name, duration);
        }
        if let Some((test_name, duration)) = &timings.shortest_test {
            tracing::warn!("Shortest: {}, Duration: {:?}", test_name, duration);
        }
        tracing::warn!("*************************************");
    }
}
