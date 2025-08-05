# Pelanca Chess Engine v1.2

Um motor de xadrez de alta performance escrito em Rust, com busca alpha-beta paralela e otimizações modernas.

## Características

### 🚀 Performance
- **Busca Alpha-Beta Paralela**: Utiliza todos os cores disponíveis
- **Magic Bitboards**: Geração ultra-rápida de movimentos
- **Transposition Table**: Cache eficiente de posições (256MB padrão)
- **Busca de Quiescência**: Evita o efeito horizonte
- **Move Ordering**: MVV-LVA, killer moves, history heuristic
- **Null Move Pruning**: Poda eficiente da árvore de busca
- **Late Move Reductions**: Reduz profundidade de movimentos tardios
- **Aspiration Windows**: Busca iterativa com janelas adaptativas

### 🎯 Avaliação
- **Material**: Valores precisos para cada tipo de peça
- **Piece-Square Tables**: Posicionamento ótimo das peças
- **Estrutura de Peões**: Peões passados, isolados, dobrados
- **Mobilidade**: Avaliação de liberdade de movimento
- **Segurança do Rei**: Escudo de peões e posição do rei
- **Fase do Jogo**: Adaptação entre abertura/meio-jogo/final

### ⚡ Otimizações
- **Copy-Make**: Estrutura otimizada para make/unmake rápido
- **Zobrist Hashing Incremental**: Atualização O(1) do hash
- **Intrinsics x86/ARM**: POPCNT, BMI2, PEXT/PDEP quando disponível
- **Tabelas Pré-computadas**: Ataques de peças não-deslizantes
- **Lazy SMP**: Paralelização moderna e eficiente
- **Perft com TT**: Validação rápida de geração de movimentos

## Compilação

### Requisitos
- Rust 1.70 ou superior
- Cargo

### Compilar versão otimizada
```bash
cargo build --release --profile=release-lto
```

### Executar testes
```bash
cargo test
```

### Benchmark
```bash
cargo run --release
```

## Uso

### Interface de Linha de Comando
```bash
./target/release/pelanca_v11
```

Comandos disponíveis:
- `position [startpos|fen <fen>]` - Define posição
- `search <depth> [time <ms>] [threads <n>]` - Busca melhor movimento
- `perft <depth>` - Teste de geração de movimentos
- `eval` - Mostra avaliação da posição atual
- `moves` - Lista movimentos legais
- `bench` - Executa benchmark de performance
- `quit/exit` - Sair do programa

### Interface UCI
```bash
./target/release/pelanca_uci
```

Compatível com qualquer GUI que suporte o protocolo UCI (Arena, Cute Chess, etc).

## Performance

Resultados típicos em hardware moderno (Ryzen 5900X):

### Perft (posição inicial)
- Depth 6: ~119M nós em 1.6s (~70M nós/seg)
- Depth 7: ~3.2B nós em 19s (~167M nós/seg)
- Depth 8: ~85B nós em 245s (~346M nós/seg)

### Busca Alpha-Beta
- Depth 10-12 em posições médias em <1 segundo
- ~5-10M nós/segundo com avaliação completa
- Taxa de acerto TT: 60-80% típico

## Arquitetura

### Estrutura de Diretórios
```
src/
├── main.rs              # Interface CLI principal
├── bin/
│   └── uci.rs          # Interface UCI
├── core/
│   ├── board.rs        # Representação do tabuleiro
│   ├── types.rs        # Tipos fundamentais
│   └── zobrist.rs      # Hashing Zobrist
├── engine/
│   └── perft_tt.rs     # Transposition table para perft
├── moves/
│   ├── pawn.rs         # Movimentos de peão
│   ├── knight.rs       # Movimentos de cavalo
│   ├── sliding.rs      # Peças deslizantes (torre/bispo)
│   ├── queen.rs        # Movimentos de rainha
│   ├── king.rs         # Movimentos de rei
│   └── magic_bitboards.rs # Magic bitboards
├── search/
│   ├── alpha_beta.rs   # Algoritmo principal
│   ├── evaluation.rs   # Função de avaliação
│   ├── move_ordering.rs # Ordenação de movimentos
│   ├── transposition_table.rs # Cache de posições
│   ├── search_thread.rs # Busca paralela
│   └── quiescence.rs   # Busca de quiescência
├── utils/
│   └── intrinsics.rs   # Otimizações de baixo nível
└── profiling/
    └── mod.rs          # Sistema de profiling
```

### Representação do Tabuleiro
- **Bitboards**: 6 bitboards para tipos de peças + 2 para cores
- **Copy-Make**: Estrutura copiável para paralelização eficiente
- **Zobrist Incremental**: Hash atualizado incrementalmente

### Geração de Movimentos
- **Magic Bitboards**: Para peças deslizantes (torre/bispo/rainha)
- **Tabelas Pré-computadas**: Para peão/cavalo/rei
- **Geração Pseudo-legal**: Validação lazy para performance

### Busca
- **Iterative Deepening**: Profundidade incremental
- **Aspiration Windows**: Janelas adaptativas
- **Principal Variation Search**: Busca eficiente da PV
- **Null Move Pruning**: R=3 adaptativo
- **Late Move Reductions**: 1-2 plys em movimentos tardios
- **Quiescence Search**: Com SEE e delta pruning

### Avaliação
- **Material + PST**: Combinados para eficiência
- **Avaliação Incremental**: Futuras otimizações
- **Fase do Jogo**: Interpolação entre meio-jogo e final
- **Tapered Eval**: Transição suave entre fases

## Contribuindo

Contribuições são bem-vindas! Por favor:
1. Fork o projeto
2. Crie sua feature branch (`git checkout -b feature/AmazingFeature`)
3. Commit suas mudanças (`git commit -m 'Add some AmazingFeature'`)
4. Push para a branch (`git push origin feature/AmazingFeature`)
5. Abra um Pull Request

## Melhorias Futuras

- [ ] Syzygy Tablebases para finais
- [ ] Avaliação com NNUE (redes neurais)
- [ ] Livro de aberturas
- [ ] Análise de variantes
- [ ] Ponder (pensar no tempo do oponente)
- [ ] Multi-PV (múltiplas variações principais)
- [ ] Singular Extensions
- [ ] Futility Pruning aprimorado
- [ ] Counter Move Heuristic
- [ ] Avaliação incremental completa

## Licença

Este projeto está licenciado sob a MIT License.

## Autor

Pedro Contessoto

## Agradecimentos

- Chess Programming Wiki
- Stockfish (inspiração para técnicas modernas)
- Rust Community
