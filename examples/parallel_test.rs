use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::{Duration, Instant};
use std::thread;

// Test whether Salsa can parallelize queries within a query
// Expected behavior: If Salsa supports in-query parallelism, independent
// queries called from the same parent should run in parallel.

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

// Define a trait for our database with access to the counter
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

    // Simulate work with a sleep
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

/// Intermediate query that depends on multiple base queries
/// This is the KEY TEST: will these 3 calls run in parallel or sequentially?
#[salsa::tracked]
fn aggregate_query(
    db: &dyn Db,
    input1: InputData,
    input2: InputData,
    input3: InputData,
) -> String {
    println!(
        "\n[{:?}] aggregate_query START - will call 3 slow_computations",
        thread::current().id(),
    );
    let start = Instant::now();

    // This is the key test: do these queries run in parallel or sequentially?
    let result1 = slow_computation(db, input1);
    let result2 = slow_computation(db, input2);
    let result3 = slow_computation(db, input3);

    let elapsed = start.elapsed();
    println!(
        "[{:?}] aggregate_query END - took {:?}",
        thread::current().id(),
        elapsed
    );

    format!("{}, {}, {}", result1, result2, result3)
}

/// Top-level query that orchestrates multiple aggregate queries
#[salsa::tracked]
fn orchestrator_query(
    db: &dyn Db,
    group1_1: InputData,
    group1_2: InputData,
    group1_3: InputData,
    group2_1: InputData,
    group2_2: InputData,
    group2_3: InputData,
) -> String {
    println!(
        "\n[{:?}] orchestrator_query START - will call 2 aggregate_queries",
        thread::current().id(),
    );
    let start = Instant::now();

    // Do these aggregate queries run in parallel?
    let result1 = aggregate_query(db, group1_1, group1_2, group1_3);
    let result2 = aggregate_query(db, group2_1, group2_2, group2_3);

    let elapsed = start.elapsed();
    println!(
        "[{:?}] orchestrator_query END - took {:?}\n",
        thread::current().id(),
        elapsed
    );

    format!("Group1: [{}] | Group2: [{}]", result1, result2)
}

fn main() {
    println!("=== Testing Salsa In-Query Parallelism ===\n");
    println!("This test creates a hierarchy of queries:");
    println!("  orchestrator_query");
    println!("    -> aggregate_query #1 (calls 3 slow_computations)");
    println!("    -> aggregate_query #2 (calls 3 slow_computations)");
    println!("\nEach slow_computation takes ~500ms.");
    println!("If queries run in parallel, we should see:");
    println!("  - Multiple 'active queries' > 1 simultaneously");
    println!("  - aggregate_query taking ~500ms (3 queries in parallel)");
    println!("  - Total time much less than 3000ms (6 × 500ms)\n");
    println!("If queries run sequentially:");
    println!("  - 'active queries' always = 1");
    println!("  - aggregate_query taking ~1500ms (3 × 500ms)");
    println!("  - Total time ~3000ms\n");

    let db = Database::new();

    // Create test data
    let input1 = InputData::new(&db, 1, "a".to_string());
    let input2 = InputData::new(&db, 2, "b".to_string());
    let input3 = InputData::new(&db, 3, "c".to_string());
    let input4 = InputData::new(&db, 4, "d".to_string());
    let input5 = InputData::new(&db, 5, "e".to_string());
    let input6 = InputData::new(&db, 6, "f".to_string());

    println!("Starting orchestrator with 2 groups of 3 queries each...\n");
    let overall_start = Instant::now();

    let result = orchestrator_query(&db, input1, input2, input3, input4, input5, input6);

    let total_elapsed = overall_start.elapsed();

    println!("=== Result ===");
    println!("{}", result);

    println!("\n=== Analysis ===");
    println!("Total time: {:?}", total_elapsed);
    println!("\nExpected timings:");
    println!("  - If fully sequential: ~3000ms (6 queries × 500ms)");
    println!("  - If parallel within aggregate_query: ~1000ms (2 groups × 500ms, since 3 run in parallel)");
    println!("  - If fully parallel: ~500ms (all 6 queries in parallel)");

    if total_elapsed < Duration::from_millis(2000) {
        println!("\n✓ Some parallelism detected!");
    } else if total_elapsed >= Duration::from_millis(2800) {
        println!("\n✗ Queries run sequentially - NO in-query parallelism");
    } else {
        println!("\n? Partial parallelism (between groups but not within)");
    }
}
