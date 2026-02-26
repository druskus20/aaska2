use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Instant;
use tokio::time::Duration;

// Test whether Picante can parallelize queries within a query
// Expected behavior: If Picante supports in-query parallelism, independent
// queries called from the same parent should run in parallel.

#[picante::input]
struct InputData {
    #[key]
    id: u32,
    value: String,
}

/// Base query that simulates some work
#[picante::tracked]
async fn slow_computation<DB: DatabaseTrait + DbExt>(db: &DB, input: InputData) -> picante::PicanteResult<String> {
    let id = *input.id(db)?;
    let value = input.value(db)?.clone();

    let active = db.active_queries().fetch_add(1, Ordering::SeqCst) + 1;
    let start = Instant::now();

    println!(
        "[task] slow_computation({}) START - active queries: {}",
        id,
        active
    );

    // Simulate work with an async sleep
    tokio::time::sleep(Duration::from_millis(500)).await;

    let result = format!("processed-{}-{}", id, value);
    let elapsed = start.elapsed();

    db.active_queries().fetch_sub(1, Ordering::SeqCst);

    println!(
        "[task] slow_computation({}) END - took {:?}",
        id,
        elapsed
    );

    Ok(result)
}

/// Intermediate query that depends on multiple base queries
/// This is the KEY TEST: will these 3 calls run in parallel or sequentially?
#[picante::tracked]
async fn aggregate_query<DB: DatabaseTrait>(
    db: &DB,
    input1: InputData,
    input2: InputData,
    input3: InputData,
) -> picante::PicanteResult<String> {
    println!("\n[task] aggregate_query START - will call 3 slow_computations");
    let start = Instant::now();

    // This is the key test: do these queries run in parallel or sequentially?
    // With tokio, we can use join! to run them truly in parallel
    let (result1, result2, result3) = tokio::join!(
        slow_computation(db, input1),
        slow_computation(db, input2),
        slow_computation(db, input3)
    );

    let elapsed = start.elapsed();
    println!("[task] aggregate_query END - took {:?}", elapsed);

    Ok(format!("{}, {}, {}", result1?, result2?, result3?))
}

/// Top-level query that orchestrates multiple aggregate queries
#[picante::tracked]
async fn orchestrator_query<DB: DatabaseTrait>(
    db: &DB,
    group1_1: InputData,
    group1_2: InputData,
    group1_3: InputData,
    group2_1: InputData,
    group2_2: InputData,
    group2_3: InputData,
) -> picante::PicanteResult<String> {
    println!("\n[task] orchestrator_query START - will call 2 aggregate_queries");
    let start = Instant::now();

    // Do these aggregate queries run in parallel? With tokio::join! they should!
    let (result1, result2) = tokio::join!(
        aggregate_query(db, group1_1, group1_2, group1_3),
        aggregate_query(db, group2_1, group2_2, group2_3)
    );

    let elapsed = start.elapsed();
    println!("[task] orchestrator_query END - took {:?}\n", elapsed);

    Ok(format!("Group1: [{}] | Group2: [{}]", result1?, result2?))
}

// Database definition must come after tracked functions
#[picante::db(inputs(InputData), tracked(slow_computation, aggregate_query, orchestrator_query))]
pub struct Database {
    pub active_queries: Arc<AtomicUsize>,
}

// Extend DatabaseTrait with active_queries access
trait DbExt: DatabaseTrait {
    fn active_queries(&self) -> &Arc<AtomicUsize>;
}

impl DbExt for Database {
    fn active_queries(&self) -> &Arc<AtomicUsize> {
        &self.active_queries
    }
}

impl DbExt for DatabaseSnapshot {
    fn active_queries(&self) -> &Arc<AtomicUsize> {
        // Snapshots don't track active queries - return a dummy counter
        // This is fine since we're not creating snapshots in this test
        static DUMMY: std::sync::OnceLock<Arc<AtomicUsize>> = std::sync::OnceLock::new();
        DUMMY.get_or_init(|| Arc::new(AtomicUsize::new(0)))
    }
}

impl Database {
    pub fn new_test() -> Self {
        Self::new(Arc::new(AtomicUsize::new(0)))
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("=== Testing Picante + Tokio In-Query Parallelism ===\n");
    println!("This test creates a hierarchy of queries:");
    println!("  orchestrator_query");
    println!("    -> aggregate_query #1 (calls 3 slow_computations)");
    println!("    -> aggregate_query #2 (calls 3 slow_computations)");
    println!("\nEach slow_computation takes ~500ms.");
    println!("With tokio::join!, queries should run in parallel:");
    println!("  - Multiple 'active queries' > 1 simultaneously");
    println!("  - aggregate_query taking ~500ms (3 queries in parallel)");
    println!("  - Total time ~500ms (all 6 queries in parallel)\n");
    println!("If queries run sequentially:");
    println!("  - 'active queries' always = 1");
    println!("  - aggregate_query taking ~1500ms (3 × 500ms)");
    println!("  - Total time ~3000ms\n");

    let db = Database::new_test();

    // Create test data
    let input1 = InputData::new(&db, 1, "a".to_string())?;
    let input2 = InputData::new(&db, 2, "b".to_string())?;
    let input3 = InputData::new(&db, 3, "c".to_string())?;
    let input4 = InputData::new(&db, 4, "d".to_string())?;
    let input5 = InputData::new(&db, 5, "e".to_string())?;
    let input6 = InputData::new(&db, 6, "f".to_string())?;

    println!("Starting orchestrator with 2 groups of 3 queries each...\n");
    let overall_start = Instant::now();

    let result = orchestrator_query(&db, input1, input2, input3, input4, input5, input6).await?;

    let total_elapsed = overall_start.elapsed();

    println!("=== Result ===");
    println!("{}", result);

    println!("\n=== Analysis ===");
    println!("Total time: {:?}", total_elapsed);
    println!("\nExpected timings:");
    println!("  - If fully sequential: ~3000ms (6 queries × 500ms)");
    println!("  - If parallel within aggregate_query: ~1000ms (2 groups × 500ms, since 3 run in parallel)");
    println!("  - If fully parallel: ~500ms (all 6 queries in parallel)");

    if total_elapsed < Duration::from_millis(800) {
        println!("\n✓ Full parallelism achieved! All 6 queries ran concurrently!");
    } else if total_elapsed < Duration::from_millis(1500) {
        println!("\n✓ Good parallelism! Queries within groups ran in parallel!");
    } else if total_elapsed < Duration::from_millis(2500) {
        println!("\n? Partial parallelism (between groups but not within)");
    } else {
        println!("\n✗ Queries run sequentially - NO in-query parallelism");
    }

    Ok(())
}
