// Sistema de profiling e análise de performance para detectar gargalos

use std::time::{Duration, Instant};
use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Mutex;

/// Medidor de performance global
pub struct PerformanceProfiler {
    timers: Mutex<HashMap<String, TimerStats>>,
    counters: Mutex<HashMap<String, AtomicU64>>,
    enabled: bool,
}

#[derive(Debug, Clone)]
struct TimerStats {
    total_time: Duration,
    call_count: u64,
    max_time: Duration,
    min_time: Duration,
}

impl TimerStats {
    fn new() -> Self {
        TimerStats {
            total_time: Duration::new(0, 0),
            call_count: 0,
            max_time: Duration::new(0, 0),
            min_time: Duration::new(u64::MAX, 0),
        }
    }

    fn record(&mut self, duration: Duration) {
        self.total_time += duration;
        self.call_count += 1;
        if duration > self.max_time {
            self.max_time = duration;
        }
        if duration < self.min_time {
            self.min_time = duration;
        }
    }

    fn average(&self) -> Duration {
        if self.call_count > 0 {
            self.total_time / self.call_count as u32
        } else {
            Duration::new(0, 0)
        }
    }
}

impl PerformanceProfiler {
    pub fn new() -> Self {
        PerformanceProfiler {
            timers: Mutex::new(HashMap::new()),
            counters: Mutex::new(HashMap::new()),
            enabled: true,
        }
    }

    pub fn enable(&mut self) {
        self.enabled = true;
    }

    pub fn disable(&mut self) {
        self.enabled = false;
    }

    /// Inicia timer para uma função/operação
    pub fn start_timer(&self, name: &str) -> TimerHandle {
        TimerHandle {
            name: if self.enabled { name.to_string() } else { String::new() },
            start: Instant::now(),
            profiler: self,
        }
    }

    /// Registra tempo de execução
    fn record_time(&self, name: &str, duration: Duration) {
        if !self.enabled {
            return;
        }

        let mut timers = self.timers.lock().unwrap();
        let stats = timers.entry(name.to_string()).or_insert_with(TimerStats::new);
        stats.record(duration);
    }

    /// Incrementa contador
    pub fn increment_counter(&self, name: &str) {
        self.add_to_counter(name, 1);
    }

    /// Adiciona valor ao contador
    pub fn add_to_counter(&self, name: &str, value: u64) {
        if !self.enabled {
            return;
        }

        let mut counters = self.counters.lock().unwrap();
        let counter = counters.entry(name.to_string())
            .or_insert_with(|| AtomicU64::new(0));
        counter.fetch_add(value, Ordering::Relaxed);
    }

    /// Gera relatório de performance (versão simplificada para evitar loops)
    pub fn generate_report(&self) -> String {
        let mut report = String::new();
        report.push_str("=== RELATÓRIO DE PERFORMANCE ===\n\n");

        // Tenta obter timers com timeout
        if let Ok(timers) = self.timers.try_lock() {
            report.push_str("TEMPOS DE EXECUÇÃO:\n");
            let mut timer_vec: Vec<_> = timers.iter().take(10).collect(); // Limita a 10 entradas
            timer_vec.sort_by(|a, b| b.1.total_time.cmp(&a.1.total_time));

            for (name, stats) in timer_vec {
                report.push_str(&format!(
                    "- {}: {}ms total, {} chamadas ({}μs média)\n",
                    truncate_string(name, 30),
                    stats.total_time.as_millis(),
                    stats.call_count,
                    stats.average().as_micros()
                ));
            }
            report.push_str("\n");
        } else {
            report.push_str("TIMERS: Bloqueado ou em uso\n\n");
        }

        // Tenta obter contadores com timeout
        if let Ok(counters) = self.counters.try_lock() {
            report.push_str("CONTADORES:\n");
            let mut counter_vec: Vec<_> = counters.iter().take(10).collect(); // Limita a 10 entradas
            counter_vec.sort_by(|a, b| {
                b.1.load(Ordering::Relaxed).cmp(&a.1.load(Ordering::Relaxed))
            });

            for (name, counter) in counter_vec {
                report.push_str(&format!(
                    "- {}: {}\n",
                    truncate_string(name, 30),
                    counter.load(Ordering::Relaxed)
                ));
            }
            report.push_str("\n");
        } else {
            report.push_str("CONTADORES: Bloqueado ou em uso\n\n");
        }

        // Análise simplificada
        report.push_str("ANÁLISE RÁPIDA:\n");
        report.push_str("- Use os dados acima para identificar gargalos\n");
        report.push_str("- Funções com mais tempo total são candidatas a otimização\n");
        report.push_str("- Funções com muitas chamadas podem se beneficiar de cache\n\n");

        report
    }

    /// Analisa gargalos e gera recomendações (versão simplificada)
    fn analyze_bottlenecks(&self, report: &mut String) {
        // Versão simplificada para evitar deadlocks
        report.push_str("DICAS DE OTIMIZAÇÃO:\n");
        report.push_str("- Verifique funções com >100ms de tempo total\n");
        report.push_str("- Funções com >10000 chamadas podem ser otimizadas\n");
        report.push_str("- Use cache para avaliações repetitivas\n");
        report.push_str("- Otimize geração de movimentos se necessário\n\n");
    }

    /// Limpa todas as estatísticas
    pub fn clear(&self) {
        self.timers.lock().unwrap().clear();
        self.counters.lock().unwrap().clear();
    }

    /// Salva relatório em arquivo
    pub fn save_report(&self, filename: &str) -> Result<(), std::io::Error> {
        use std::fs::File;
        use std::io::Write;

        println!("info string Iniciando geração do relatório...");
        let report = self.generate_report();
        println!("info string Relatório gerado com sucesso. Iniciando salvamento em {}...", filename);

        let mut file = File::create(filename)?;
        file.write_all(report.as_bytes())?;

        println!("info string Relatório salvo com sucesso em: {}", filename);
        Ok(())
    }
}

/// Handle para medir tempo automaticamente
pub struct TimerHandle<'a> {
    name: String,
    start: Instant,
    profiler: &'a PerformanceProfiler,
}

impl<'a> Drop for TimerHandle<'a> {
    fn drop(&mut self) {
        if !self.name.is_empty() {
            let duration = self.start.elapsed();
            self.profiler.record_time(&self.name, duration);
        }
    }
}

/// Instância global do profiler usando OnceLock para compatibilidade
use std::sync::OnceLock;
static PROFILER: OnceLock<PerformanceProfiler> = OnceLock::new();

/// Acessa o profiler global
pub fn get_profiler() -> &'static PerformanceProfiler {
    PROFILER.get_or_init(|| PerformanceProfiler::new())
}

/// Macro para facilitar o profiling
#[macro_export]
macro_rules! profile {
    ($name:expr, $code:block) => {
        {
            let _timer = crate::profiling::get_profiler().start_timer($name);
            $code
        }
    };
}

/// Macro para incrementar contador
#[macro_export]
macro_rules! count {
    ($name:expr) => {
        crate::profiling::PROFILER.increment_counter($name);
    };
    ($name:expr, $value:expr) => {
        crate::profiling::PROFILER.add_to_counter($name, $value);
    };
}

// Funções utilitárias
fn truncate_string(s: &str, max_len: usize) -> String {
    if s.len() <= max_len {
        s.to_string()
    } else {
        format!("{}...", &s[..max_len-3])
    }
}

/// Sistema de benchmark para operações críticas
pub struct Benchmark {
    name: String,
    iterations: u64,
    total_time: Duration,
}

impl Benchmark {
    pub fn new(name: &str) -> Self {
        Benchmark {
            name: name.to_string(),
            iterations: 0,
            total_time: Duration::new(0, 0),
        }
    }

    /// Executa benchmark de uma função
    pub fn run<F, R>(&mut self, mut f: F, iterations: u64) -> R 
    where
        F: FnMut() -> R,
    {
        let start = Instant::now();
        let mut result = None;
        
        for _ in 0..iterations {
            result = Some(f());
        }
        
        let elapsed = start.elapsed();
        self.iterations += iterations;
        self.total_time += elapsed;
        
        println!("info string Benchmark {}: {} iterações em {}ms ({}ns por iteração)",
                 self.name, 
                 iterations, 
                 elapsed.as_millis(),
                 elapsed.as_nanos() / iterations as u128);
        
        result.unwrap()
    }

    /// Compara performance entre duas implementações
    pub fn compare<F1, F2, R>(name1: &str, f1: F1, name2: &str, f2: F2, iterations: u64)
    where
        F1: Fn() -> R,
        F2: Fn() -> R,
    {
        println!("info string Comparando performance: {} vs {}", name1, name2);
        
        // Aquece o cache
        for _ in 0..10 {
            f1();
            f2();
        }
        
        // Benchmark da primeira função
        let start1 = Instant::now();
        for _ in 0..iterations {
            f1();
        }
        let time1 = start1.elapsed();
        
        // Benchmark da segunda função
        let start2 = Instant::now();
        for _ in 0..iterations {
            f2();
        }
        let time2 = start2.elapsed();
        
        // Resultados
        let ratio = if time2.as_nanos() > 0 {
            time1.as_nanos() as f64 / time2.as_nanos() as f64
        } else {
            1.0
        };
        
        println!("info string {} vs {} - Resultados:", name1, name2);
        println!("info string   {}: {}ms ({}ns por operação)", 
                 name1, time1.as_millis(), time1.as_nanos() / iterations as u128);
        println!("info string   {}: {}ms ({}ns por operação)", 
                 name2, time2.as_millis(), time2.as_nanos() / iterations as u128);
        
        if ratio > 1.1 {
            println!("info string   {} é {:.1}x mais rápido que {}", name2, ratio, name1);
        } else if ratio < 0.9 {
            println!("info string   {} é {:.1}x mais rápido que {}", name1, 1.0/ratio, name2);
        } else {
            println!("info string   Performance similar entre {} e {}", name1, name2);
        }
    }
}