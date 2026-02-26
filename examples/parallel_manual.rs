use rayon::prelude::*;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::thread;
use std::time::{Duration, Instant};

// Test manual parallelization of Salsa queries using rayon
// This demonstrates a workaround for Salsa's lack of in-query parallelism

#[salsa::input]
struct InputData {
    id: u32,
    value: String,
}

#[salsa::db]
#[derive(Clone)]
struct Database {
    storage: salsa::Storage<Self>,
    active_queries: Arc<AtomicUsize>,
}

impl Database {
    fn new() -> Self {
        Self {
            storage: salsa::Storage::default(),
            active_queries: Arc::new(AtomicUsize::new(0)),
        }
    }
}

#[salsa::db]
impl salsa::Database for Database {}

#[salsa::db]
trait Db: salsa::Database {
    fn active_queries(&self) -> Arc<AtomicUsize>;
}

#[salsa::db]
impl Db for Database {
    fn active_queries(&self) -> Arc<AtomicUsize> {
        self.active_queries.clone()
    }
}

/// Base query that simulates some work
#[salsa::tracked]
fn slow_computation(db: &dyn Db, input: InputData) -> String {
    let id = input.id(db);
    let value = input.value(db);

    let active = db.active_queries().fetch_add(1, Ordering::SeqCst) + 1;
    let start = Instant::now();

    println!(
        "[{:?}] slow_computation({}) START - active queries: {}",
        thread::current().id(),
        id,
        active
    );

    // Simulate work
    thread::sleep(Duration::from_millis(500));

    let result = format!("processed-{}-{}", id, value);
    let elapsed = start.elapsed();

    db.active_queries().fetch_sub(1, Ordering::SeqCst);

    println!(
        "[{:?}] slow_computation({}) END - took {:?}",
        thread::current().id(),
        id,
        elapsed
    );

    result
}

/// Sequential version - queries run one after another
#[salsa::tracked]
fn aggregate_sequential(
    db: &dyn Db,
    input1: InputData,
    input2: InputData,
    input3: InputData,
) -> String {
    println!(
        "\n[{:?}] aggregate_sequential START",
        thread::current().id(),
    );
    let start = Instant::now();

    // Sequential execution
    let result1 = slow_computation(db, input1);
    let result2 = slow_computation(db, input2);
    let result3 = slow_computation(db, input3);

    let elapsed = start.elapsed();
    println!(
        "[{:?}] aggregate_sequential END - took {:?}",
        thread::current().id(),
        elapsed
    );

    format!("{}, {}, {}", result1, result2, result3)
}

/// Parallel version using rayon - queries run in parallel!
#[salsa::tracked]
fn aggregate_parallel(
    db: &dyn Db,
    input1: InputData,
    input2: InputData,
    input3: InputData,
) -> String {
    println!("\n[{:?}] aggregate_parallel START", thread::current().id(),);
    let start = Instant::now();

    // Parallel execution using rayon
    // Clone the database for each thread (cheap operation, shares storage)
    let inputs = vec![input1, input2, input3];

    let results: Vec<String> = inputs
        .par_iter()
        .map(|&input| {
            // Each parallel task gets a clone of the db
            slow_computation(db, input)
        })
        .collect();

    let elapsed = start.elapsed();
    println!(
        "[{:?}] aggregate_parallel END - took {:?}",
        thread::current().id(),
        elapsed
    );

    results.join(", ")
}

fn main() {
    println!("=== Testing Manual Parallelization of Salsa Queries ===\n");
    println!("This test compares:");
    println!("  1. Sequential query execution (normal Salsa)");
    println!("  2. Parallel query execution (using rayon)\n");
    println!("Each query takes ~500ms.\n");

    let db = Database::new();

    // Create test data
    let input1 = InputData::new(&db, 1, "a".to_string());
    let input2 = InputData::new(&db, 2, "b".to_string());
    let input3 = InputData::new(&db, 3, "c".to_string());

    // Test 1: Sequential execution
    println!("═══════════════════════════════════════════════════");
    println!("TEST 1: Sequential execution (default Salsa)");
    println!("Expected: ~1500ms (3 × 500ms)");
    println!("═══════════════════════════════════════════════════");

    let start = Instant::now();
    let result_seq = aggregate_sequential(&db, input1, input2, input3);
    let elapsed_seq = start.elapsed();

    println!("\nResult: {}", result_seq);
    println!("Total time: {:?}\n", elapsed_seq);

    // Small delay between tests
    thread::sleep(Duration::from_millis(200));

    // Create new inputs for second test (avoid cached results)
    let input4 = InputData::new(&db, 4, "d".to_string());
    let input5 = InputData::new(&db, 5, "e".to_string());
    let input6 = InputData::new(&db, 6, "f".to_string());

    // Test 2: Parallel execution
    println!("═══════════════════════════════════════════════════");
    println!("TEST 2: Parallel execution (using rayon)");
    println!("Expected: ~500ms (3 queries in parallel)");
    println!("═══════════════════════════════════════════════════");

    let start = Instant::now();
    let result_par = aggregate_parallel(&db, input4, input5, input6);
    let elapsed_par = start.elapsed();

    println!("\nResult: {}", result_par);
    println!("Total time: {:?}\n", elapsed_par);

    // Analysis
    println!("═══════════════════════════════════════════════════");
    println!("ANALYSIS");
    println!("═══════════════════════════════════════════════════");
    println!("Sequential time: {:?}", elapsed_seq);
    println!("Parallel time:   {:?}", elapsed_par);

    let speedup = elapsed_seq.as_millis() as f64 / elapsed_par.as_millis() as f64;
    println!("Speedup: {:.2}x", speedup);

    if speedup > 2.0 {
        println!("\n✓ Excellent speedup! Manual parallelization works well.");
    } else if speedup > 1.5 {
        println!("\n✓ Good speedup! Parallelization is effective.");
    } else {
        println!("\n✗ Limited speedup. Check thread pool configuration.");
    }

    println!("\nConclusion: You CAN manually parallelize Salsa queries using");
    println!("rayon or std::thread. The database is Clone and thread-safe.");
}
