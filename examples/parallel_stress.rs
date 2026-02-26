use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Mutex;
use std::time::Instant;
use tokio::time::Duration;
use std::collections::{HashMap, HashSet};
use std::fs::File;
use std::io::Write;

// HARDCORE PARALLELISM STRESS TEST
// Tests deep nesting, hundreds of concurrent queries, and extreme parallelism
// Generates a dependency graph visualization

lazy_static::lazy_static! {
    static ref DEPENDENCY_GRAPH: Mutex<HashMap<String, HashSet<String>>> = Mutex::new(HashMap::new());
}

fn track_dependency(parent: &str, child: &str) {
    let mut graph = DEPENDENCY_GRAPH.lock().unwrap();
    graph.entry(parent.to_string()).or_insert_with(HashSet::new).insert(child.to_string());
}

fn generate_dot_graph(filename: &str, total_queries: usize, peak: usize, elapsed_ms: u128) -> std::io::Result<()> {
    let graph = DEPENDENCY_GRAPH.lock().unwrap();

    let mut dot = String::new();
    dot.push_str("digraph StressTest {\n");
    dot.push_str("  rankdir=TB;\n");
    dot.push_str("  node [shape=box, style=filled, fontsize=10];\n");
    dot.push_str("  edge [arrowsize=0.5];\n");
    dot.push_str("  graph [fontsize=16, ranksep=1.5, nodesep=0.5];\n\n");

    // Add title
    dot.push_str(&format!("  labelloc=\"t\";\n"));
    dot.push_str(&format!("  label=\"Parallel Stress Test (Simplified View)\\n{} total queries, {} peak concurrency, {}ms\";\n\n",
        total_queries, peak, elapsed_ms));

    // Create abstract/summary nodes instead of showing all 3000
    dot.push_str("  // Top level\n");
    dot.push_str("  \"ultra_mega_query\" [fillcolor=\"#FF6347\", label=\"Ultra Mega Query\\n(3 batches)\", fontsize=14, width=2.5];\n\n");

    dot.push_str("  // Mega level (3 nodes, one per batch)\n");
    for i in 0..3 {
        dot.push_str(&format!("  \"mega_{}\" [fillcolor=\"#FFD700\", label=\"Mega Query {}\\n(1000 leaves)\"];\n", i, i));
    }
    dot.push_str("\n");

    dot.push_str("  // Sample aggregate_200 nodes (show 2 per mega)\n");
    for i in 0..3 {
        for j in 0..2 {
            dot.push_str(&format!("  \"agg200_{}_{}\" [fillcolor=\"#DDA0DD\", label=\"Aggregate 200\\n(batch {}, #{})\"];\n", i, j, i, j));
        }
    }
    dot.push_str("\n");

    dot.push_str("  // Sample aggregate_50 nodes (show 1 per agg200)\n");
    for i in 0..3 {
        for j in 0..2 {
            dot.push_str(&format!("  \"agg50_{}_{}\" [fillcolor=\"#87CEEB\", label=\"Aggregate 50\\n(5 groups)\"];\n", i, j));
        }
    }
    dot.push_str("\n");

    dot.push_str("  // Sample aggregate_10 nodes\n");
    for i in 0..2 {
        dot.push_str(&format!("  \"agg10_{}\" [fillcolor=\"#90EE90\", label=\"Aggregate 10\\n(10 leaves)\"];\n", i));
    }
    dot.push_str("\n");

    dot.push_str("  // Sample leaf nodes\n");
    for i in 0..5 {
        dot.push_str(&format!("  \"leaf_{}\" [fillcolor=\"#FFB6C1\", label=\"Leaf Work\\n(10-50ms)\", fontsize=9];\n", i));
    }
    dot.push_str("\n");

    dot.push_str("  // Ellipsis nodes to show continuation\n");
    dot.push_str("  \"more_mega\" [label=\"...\", shape=plaintext, fontsize=20];\n");
    dot.push_str("  \"more_agg200\" [label=\"... (18 more)\", shape=plaintext, fontsize=12];\n");
    dot.push_str("  \"more_agg50\" [label=\"... (many more)\", shape=plaintext, fontsize=10];\n");
    dot.push_str("  \"more_agg10\" [label=\"... (hundreds more)\", shape=plaintext, fontsize=10];\n");
    dot.push_str("  \"more_leaves\" [label=\"... (2995 more leaves)\", shape=plaintext, fontsize=10];\n\n");

    // Edges - show representative structure
    dot.push_str("  // Top to mega\n");
    for i in 0..3 {
        dot.push_str(&format!("  \"ultra_mega_query\" -> \"mega_{}\";\n", i));
    }

    dot.push_str("\n  // Mega to aggregate_200 (sample)\n");
    for i in 0..3 {
        for j in 0..2 {
            dot.push_str(&format!("  \"mega_{}\" -> \"agg200_{}_{}\";\n", i, i, j));
        }
        if i == 1 {
            dot.push_str(&format!("  \"mega_{}\" -> \"more_agg200\" [style=dashed];\n", i));
        }
    }

    dot.push_str("\n  // Aggregate_200 to aggregate_50 (sample)\n");
    for i in 0..3 {
        for j in 0..2 {
            dot.push_str(&format!("  \"agg200_{}_{}\" -> \"agg50_{}_{}\";\n", i, j, i, j));
        }
    }
    dot.push_str("  \"agg50_0_0\" -> \"more_agg50\" [style=dashed];\n");

    dot.push_str("\n  // Aggregate_50 to aggregate_10 (sample)\n");
    for i in 0..2 {
        dot.push_str(&format!("  \"agg50_0_0\" -> \"agg10_{}\";\n", i));
    }
    dot.push_str("  \"agg50_0_0\" -> \"more_agg10\" [style=dashed];\n");

    dot.push_str("\n  // Aggregate_10 to leaves (sample)\n");
    for i in 0..5 {
        dot.push_str(&format!("  \"agg10_0\" -> \"leaf_{}\";\n", i));
    }
    dot.push_str("  \"agg10_0\" -> \"more_leaves\" [style=dashed];\n");

    dot.push_str("\n  // Legend\n");
    dot.push_str("  subgraph cluster_legend {\n");
    dot.push_str("    label=\"Query Hierarchy\\n(Showing representative sample)\";\n");
    dot.push_str("    style=filled;\n");
    dot.push_str("    color=lightgrey;\n");
    dot.push_str("    node [fontsize=10];\n");
    dot.push_str("    \"L_leaf\" [label=\"Leaf (10-50ms)\", fillcolor=\"#FFB6C1\"];\n");
    dot.push_str("    \"L_10\" [label=\"Agg10 (10 leaves)\", fillcolor=\"#90EE90\"];\n");
    dot.push_str("    \"L_50\" [label=\"Agg50 (50 leaves)\", fillcolor=\"#87CEEB\"];\n");
    dot.push_str("    \"L_200\" [label=\"Agg200 (200 leaves)\", fillcolor=\"#DDA0DD\"];\n");
    dot.push_str("    \"L_mega\" [label=\"Mega (1000 leaves)\", fillcolor=\"#FFD700\"];\n");
    dot.push_str("    \"L_ultra\" [label=\"Ultra (3000 leaves)\", fillcolor=\"#FF6347\"];\n");
    dot.push_str("  }\n");

    dot.push_str("\n  // Statistics box\n");
    dot.push_str("  subgraph cluster_stats {\n");
    dot.push_str("    label=\"Performance\";\n");
    dot.push_str("    style=filled;\n");
    dot.push_str("    color=lightyellow;\n");
    dot.push_str(&format!("    \"stats\" [shape=box, label=\"Total: {} queries\\nPeak: {} concurrent\\nTime: {}ms\\nQPS: {:.0}\", fontsize=11];\n",
        total_queries, peak, elapsed_ms, total_queries as f64 / (elapsed_ms as f64 / 1000.0)));
    dot.push_str("  }\n");

    dot.push_str("}\n");

    let mut file = File::create(filename)?;
    file.write_all(dot.as_bytes())?;

    println!("\n📊 Simplified dependency graph written to {}", filename);
    println!("   (Showing representative structure, not all {} nodes)", total_queries);
    println!("   To generate PNG: dot -Tpng {} -o stress_graph.png", filename);

    Ok(())
}

#[picante::input]
struct WorkItem {
    #[key]
    id: u64,
    value: String,
    weight: u32, // milliseconds to simulate
}

/// Leaf query - simulates variable work
#[picante::tracked]
async fn leaf_work<DB: DatabaseTrait + DbExt>(db: &DB, item: WorkItem) -> picante::PicanteResult<u64> {
    let id = *item.id(db)?;
    let weight = item.weight(db)?.clone();

    let active = db.active_queries().fetch_add(1, Ordering::SeqCst) + 1;
    let peak = db.peak_queries().fetch_max(active, Ordering::SeqCst).max(active);

    if active == peak {
        println!("  [NEW PEAK] {} concurrent queries!", active);
    }

    tokio::time::sleep(Duration::from_millis(weight as u64)).await;

    db.active_queries().fetch_sub(1, Ordering::SeqCst);

    Ok(id * (weight as u64))
}

/// Level 1: Aggregates 10 leaf queries
#[picante::tracked]
async fn aggregate_10<DB: DatabaseTrait + DbExt>(
    db: &DB,
    items: Vec<WorkItem>
) -> picante::PicanteResult<u64> {
    let ids: Vec<u64> = items.iter().map(|item| *item.id(db).unwrap()).collect();
    let agg_name = format!("aggregate_10({:?})", &ids[0..ids.len().min(3)]);

    for id in &ids {
        track_dependency(&agg_name, &format!("leaf_work({})", id));
    }

    let futures: Vec<_> = items.into_iter().map(|item| leaf_work(db, item)).collect();
    let results = futures::future::join_all(futures).await;

    let mut sum = 0u64;
    for result in results {
        sum += result?;
    }
    Ok(sum)
}

/// Level 2: Aggregates 5 Level-1 aggregates (50 leaf queries total)
#[picante::tracked]
async fn aggregate_50<DB: DatabaseTrait + DbExt>(
    db: &DB,
    groups: Vec<Vec<WorkItem>>
) -> picante::PicanteResult<u64> {
    let agg_name = format!("aggregate_50({}grps)", groups.len());

    for (i, group) in groups.iter().enumerate() {
        if i < 2 { // Track first 2 for visualization
            let ids: Vec<u64> = group.iter().map(|item| *item.id(db).unwrap()).collect();
            track_dependency(&agg_name, &format!("aggregate_10({:?})", &ids[0..ids.len().min(3)]));
        }
    }

    let futures: Vec<_> = groups.into_iter().map(|group| aggregate_10(db, group)).collect();
    let results = futures::future::join_all(futures).await;

    let mut sum = 0u64;
    for result in results {
        sum += result?;
    }
    Ok(sum)
}

/// Level 3: Aggregates 4 Level-2 aggregates (200 leaf queries total)
#[picante::tracked]
async fn aggregate_200<DB: DatabaseTrait + DbExt>(
    db: &DB,
    super_groups: Vec<Vec<Vec<WorkItem>>>
) -> picante::PicanteResult<u64> {
    let futures: Vec<_> = super_groups.into_iter()
        .map(|groups| aggregate_50(db, groups))
        .collect();
    let results = futures::future::join_all(futures).await;

    let mut sum = 0u64;
    for result in results {
        sum += result?;
    }
    Ok(sum)
}

/// MEGA QUERY: Runs 5 Level-3 aggregates = 1000 leaf queries!
#[picante::tracked]
async fn mega_query<DB: DatabaseTrait + DbExt>(
    db: &DB,
    mega_groups: Vec<Vec<Vec<Vec<WorkItem>>>>
) -> picante::PicanteResult<u64> {
    let start = Instant::now();
    println!("\n🚀 MEGA QUERY START - launching 1000 leaf queries...\n");

    let futures: Vec<_> = mega_groups.into_iter()
        .map(|super_groups| aggregate_200(db, super_groups))
        .collect();
    let results = futures::future::join_all(futures).await;

    let mut sum = 0u64;
    for result in results {
        sum += result?;
    }

    let elapsed = start.elapsed();
    println!("\n🎉 MEGA QUERY COMPLETE - took {:?}", elapsed);

    Ok(sum)
}

/// ULTRA MEGA QUERY: Runs multiple mega queries in parallel
#[picante::tracked]
async fn ultra_mega_query<DB: DatabaseTrait + DbExt>(
    db: &DB,
    batches: Vec<Vec<Vec<Vec<Vec<WorkItem>>>>>
) -> picante::PicanteResult<Vec<u64>> {
    let start = Instant::now();
    let batch_count = batches.len();
    let total_queries = batch_count * 1000;

    println!("\n🔥🔥🔥 ULTRA MEGA QUERY START 🔥🔥🔥");
    println!("Running {} batches = {} total leaf queries!", batch_count, total_queries);
    println!("This will stress test the system to its limits!\n");

    let futures: Vec<_> = batches.into_iter()
        .map(|mega_groups| mega_query(db, mega_groups))
        .collect();
    let results = futures::future::join_all(futures).await;

    let mut sums = Vec::new();
    for result in results {
        sums.push(result?);
    }

    let elapsed = start.elapsed();
    let peak = db.peak_queries().load(Ordering::SeqCst);

    println!("\n🎊 ULTRA MEGA QUERY COMPLETE 🎊");
    println!("Total time: {:?}", elapsed);
    println!("Peak concurrent queries: {}", peak);
    println!("Total queries executed: {}", total_queries);
    println!("Queries per second: {:.0}", total_queries as f64 / elapsed.as_secs_f64());

    Ok(sums)
}

// Database definition must come after tracked functions
#[picante::db(inputs(WorkItem), tracked(leaf_work, aggregate_10, aggregate_50, aggregate_200, mega_query, ultra_mega_query))]
pub struct Database {
    pub active_queries: Arc<AtomicUsize>,
    pub peak_queries: Arc<AtomicUsize>,
}

// Extend DatabaseTrait with active_queries access
trait DbExt: DatabaseTrait {
    fn active_queries(&self) -> &Arc<AtomicUsize>;
    fn peak_queries(&self) -> &Arc<AtomicUsize>;
}

impl DbExt for Database {
    fn active_queries(&self) -> &Arc<AtomicUsize> {
        &self.active_queries
    }
    fn peak_queries(&self) -> &Arc<AtomicUsize> {
        &self.peak_queries
    }
}

impl DbExt for DatabaseSnapshot {
    fn active_queries(&self) -> &Arc<AtomicUsize> {
        static DUMMY: std::sync::OnceLock<Arc<AtomicUsize>> = std::sync::OnceLock::new();
        DUMMY.get_or_init(|| Arc::new(AtomicUsize::new(0)))
    }
    fn peak_queries(&self) -> &Arc<AtomicUsize> {
        static DUMMY: std::sync::OnceLock<Arc<AtomicUsize>> = std::sync::OnceLock::new();
        DUMMY.get_or_init(|| Arc::new(AtomicUsize::new(0)))
    }
}

impl Database {
    pub fn new_test() -> Self {
        Self::new(
            Arc::new(AtomicUsize::new(0)),
            Arc::new(AtomicUsize::new(0))
        )
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("╔════════════════════════════════════════════════════════════╗");
    println!("║  PICANTE + TOKIO HARDCORE PARALLELISM STRESS TEST         ║");
    println!("╚════════════════════════════════════════════════════════════╝\n");

    let db = Database::new_test();

    // Create test data structure:
    // 3 batches × 5 mega_groups × 4 super_groups × 5 groups × 10 items
    // = 3 × 1000 = 3000 total leaf queries

    println!("🏗️  Building test data (3000 work items)...");
    let mut id_counter = 0u64;

    let mut batches = Vec::new();
    for batch_num in 0..3 {
        let mut mega_groups = Vec::new();
        for mega_num in 0..5 {
            let mut super_groups = Vec::new();
            for super_num in 0..4 {
                let mut groups = Vec::new();
                for group_num in 0..5 {
                    let mut items = Vec::new();
                    for item_num in 0..10 {
                        // Variable workload: 10-50ms per query
                        let weight = 10 + (id_counter % 5) * 10;
                        items.push(
                            WorkItem::new(
                                &db,
                                id_counter,
                                format!("b{}m{}s{}g{}i{}", batch_num, mega_num, super_num, group_num, item_num),
                                weight as u32
                            )?
                        );
                        id_counter += 1;
                    }
                    groups.push(items);
                }
                super_groups.push(groups);
            }
            mega_groups.push(super_groups);
        }
        batches.push(mega_groups);
    }

    println!("✅ Test data built: {} work items", id_counter);
    println!("\n📊 Test Configuration:");
    println!("  • 3 batches running in parallel");
    println!("  • Each batch: 1000 leaf queries");
    println!("  • Total: 3000 concurrent queries");
    println!("  • Nesting depth: 5 levels");
    println!("  • Work per query: 10-50ms");
    println!("\n⏱️  Expected timings:");
    println!("  • If fully parallel: ~50ms (limited by longest task)");
    println!("  • If sequential: ~90 seconds (3000 × 30ms average)");
    println!("\n🚦 Starting stress test...\n");

    let overall_start = Instant::now();

    let results = ultra_mega_query(&db, batches).await?;

    let total_elapsed = overall_start.elapsed();
    let peak = db.peak_queries.load(Ordering::SeqCst);

    println!("\n╔════════════════════════════════════════════════════════════╗");
    println!("║  FINAL RESULTS                                             ║");
    println!("╚════════════════════════════════════════════════════════════╝");
    println!("\n📈 Statistics:");
    println!("  • Total time: {:?}", total_elapsed);
    println!("  • Peak concurrency: {} queries", peak);
    println!("  • Batch sums: {:?}", results);
    println!("  • Queries per second: {:.0}", 3000.0 / total_elapsed.as_secs_f64());

    println!("\n🏆 Parallelism Analysis:");
    if total_elapsed.as_millis() < 100 {
        println!("  ✅ EXCELLENT! Near-perfect parallelism achieved!");
        println!("  ✅ All 3000 queries ran concurrently!");
    } else if total_elapsed.as_millis() < 500 {
        println!("  ✅ GREAT! Strong parallelism detected!");
        println!("  ✅ Most queries ran in parallel!");
    } else if total_elapsed.as_millis() < 5000 {
        println!("  ⚠️  GOOD: Partial parallelism");
        println!("  ⚠️  Some queries waited for others");
    } else if total_elapsed.as_millis() < 30000 {
        println!("  ⚠️  LIMITED: Significant queuing");
        println!("  ⚠️  Many queries ran sequentially");
    } else {
        println!("  ❌ POOR: Mostly sequential execution");
        println!("  ❌ Queries ran one after another");
    }

    println!("\n🎯 Peak Concurrency Analysis:");
    if peak >= 2500 {
        println!("  ✅ OUTSTANDING! {} concurrent queries!", peak);
    } else if peak >= 1000 {
        println!("  ✅ EXCELLENT! {} concurrent queries!", peak);
    } else if peak >= 500 {
        println!("  ✅ GOOD! {} concurrent queries!", peak);
    } else if peak >= 100 {
        println!("  ⚠️  MODERATE: {} concurrent queries", peak);
    } else {
        println!("  ⚠️  LOW: Only {} concurrent queries", peak);
    }

    println!("\n════════════════════════════════════════════════════════════");

    // Generate dependency graph
    generate_dot_graph("stress_graph.dot", 3000, peak, total_elapsed.as_millis())?;

    println!("\n💡 To view the dependency graph:");
    println!("   dot -Tpng stress_graph.dot -o stress_graph.png");
    println!("   open stress_graph.png");

    Ok(())
}
