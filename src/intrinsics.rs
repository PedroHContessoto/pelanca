// Sistema de intrinsics otimizados para operações de bitboard
// Performance crítica: operações básicas de bitboard ultra-rápidas

use crate::types::Bitboard;

// ============================================================================
// INTRINSICS DE ALTA PERFORMANCE PARA BITBOARDS
// ============================================================================

/// Conta o número de bits setados (popcount) usando intrinsics quando disponível
#[inline(always)]
pub fn popcount(bb: Bitboard) -> u32 {
    #[cfg(target_arch = "x86_64")]
    {
        if is_x86_feature_detected!("popcnt") {
            unsafe {
                std::arch::x86_64::_popcnt64(bb as i64) as u32
            }
        } else {
            bb.count_ones()
        }
    }
    #[cfg(target_arch = "x86")]
    {
        if is_x86_feature_detected!("popcnt") {
            unsafe {
                let low = bb as u32;
                let high = (bb >> 32) as u32;
                std::arch::x86::_popcnt32(low as i32) as u32 +
                std::arch::x86::_popcnt32(high as i32) as u32
            }
        } else {
            bb.count_ones()
        }
    }
    #[cfg(target_arch = "aarch64")]
    {
        // Use AArch64 native vectorized popcount for maximum performance
        unsafe {
            let v = std::arch::aarch64::vreinterpretq_u8_u64(std::arch::aarch64::vdupq_n_u64(bb));
            let cnt = std::arch::aarch64::vcntq_u8(v);
            std::arch::aarch64::vaddlvq_u8(cnt) as u32
        }
    }
    #[cfg(not(any(target_arch = "x86_64", target_arch = "x86", target_arch = "aarch64")))]
    {
        bb.count_ones()
    }
}

/// Encontra o índice do bit menos significativo (LSB) usando intrinsics
#[inline(always)]
pub fn trailing_zeros(bb: Bitboard) -> u32 {
    #[cfg(target_arch = "x86_64")]
    {
        if is_x86_feature_detected!("bmi1") {
            unsafe {
                std::arch::x86_64::_tzcnt_u64(bb) as u32
            }
        } else {
            bb.trailing_zeros()
        }
    }
    #[cfg(target_arch = "x86")]
    {
        if is_x86_feature_detected!("bmi1") {
            unsafe {
                if bb == 0 {
                    64
                } else {
                    let low = bb as u32;
                    if low != 0 {
                        std::arch::x86::_tzcnt_u32(low)
                    } else {
                        32 + std::arch::x86::_tzcnt_u32((bb >> 32) as u32)
                    }
                }
            }
        } else {
            bb.trailing_zeros()
        }
    }
    #[cfg(target_arch = "aarch64")]
    {
        // AArch64 has excellent trailing zeros support
        if bb == 0 { 64 } else { bb.trailing_zeros() }
    }
    #[cfg(not(any(target_arch = "x86_64", target_arch = "x86", target_arch = "aarch64")))]
    {
        bb.trailing_zeros()
    }
}

/// Encontra o índice do bit mais significativo (MSB) usando intrinsics
#[inline(always)]
pub fn leading_zeros(bb: Bitboard) -> u32 {
    #[cfg(target_arch = "x86_64")]
    {
        if is_x86_feature_detected!("lzcnt") {
            unsafe {
                std::arch::x86_64::_lzcnt_u64(bb) as u32
            }
        } else {
            bb.leading_zeros()
        }
    }
    #[cfg(target_arch = "x86")]
    {
        if is_x86_feature_detected!("lzcnt") {
            unsafe {
                if bb == 0 {
                    64
                } else {
                    let high = (bb >> 32) as u32;
                    if high != 0 {
                        std::arch::x86::_lzcnt_u32(high)
                    } else {
                        32 + std::arch::x86::_lzcnt_u32(bb as u32)
                    }
                }
            }
        } else {
            bb.leading_zeros()
        }
    }
    #[cfg(target_arch = "aarch64")]
    {
        // AArch64 CLZ instruction via compiler intrinsic
        if bb == 0 { 64 } else { bb.leading_zeros() }
    }
    #[cfg(not(any(target_arch = "x86_64", target_arch = "x86", target_arch = "aarch64")))]
    {
        bb.leading_zeros()
    }
}

/// Remove e retorna o LSB (pop LSB) usando intrinsics para máxima performance
#[inline(always)]
pub fn pop_lsb(bb: &mut Bitboard) -> u32 {
    let lsb_index = trailing_zeros(*bb);
    *bb &= *bb - 1; // Remove o LSB
    lsb_index
}

/// Isola o LSB (retorna apenas o bit menos significativo)
#[inline(always)]
pub fn isolate_lsb(bb: Bitboard) -> Bitboard {
    #[cfg(target_arch = "x86_64")]
    {
        if is_x86_feature_detected!("bmi1") {
            unsafe {
                std::arch::x86_64::_blsi_u64(bb)
            }
        } else {
            bb & bb.wrapping_neg()
        }
    }
    #[cfg(not(target_arch = "x86_64"))]
    {
        bb & bb.wrapping_neg()
    }
}

/// Reset do LSB (remove o bit menos significativo) usando intrinsics
#[inline(always)]
pub fn reset_lsb(bb: Bitboard) -> Bitboard {
    #[cfg(target_arch = "x86_64")]
    {
        if is_x86_feature_detected!("bmi1") {
            unsafe {
                std::arch::x86_64::_blsr_u64(bb)
            }
        } else {
            bb & (bb - 1)
        }
    }
    #[cfg(not(target_arch = "x86_64"))]
    {
        bb & (bb - 1)
    }
}

/// Verifica se o bitboard tem apenas um bit setado (é potência de 2)
#[inline(always)]
pub fn is_single_bit(bb: Bitboard) -> bool {
    bb != 0 && (bb & (bb - 1)) == 0
}

/// Verifica se o bitboard está vazio
#[inline(always)]
pub fn is_empty(bb: Bitboard) -> bool {
    bb == 0
}

/// Verifica se o bitboard não está vazio
#[inline(always)]
pub fn is_not_empty(bb: Bitboard) -> bool {
    bb != 0
}

// ============================================================================
// OPERAÇÕES AVANÇADAS DE BITBOARD COM INTRINSICS
// ============================================================================

/// Paraleliza operações bit por bit usando PEXT/PDEP quando disponível
#[cfg(target_arch = "x86_64")]
#[inline(always)]
pub fn parallel_extract(source: Bitboard, mask: Bitboard) -> Bitboard {
    if is_x86_feature_detected!("bmi2") {
        unsafe {
            std::arch::x86_64::_pext_u64(source, mask)
        }
    } else {
        // Fallback otimizado para CPUs sem BMI2
        let mut result = 0u64;
        let mut bit_pos = 0;
        let mut m = mask;
        
        while m != 0 {
            let lsb = m & m.wrapping_neg();
            if (source & lsb) != 0 {
                result |= 1u64 << bit_pos;
            }
            bit_pos += 1;
            m &= m - 1;
        }
        result
    }
}

/// Paraleliza depósito de bits usando PDEP quando disponível
#[cfg(target_arch = "x86_64")]
#[inline(always)]
pub fn parallel_deposit(source: Bitboard, mask: Bitboard) -> Bitboard {
    if is_x86_feature_detected!("bmi2") {
        unsafe {
            std::arch::x86_64::_pdep_u64(source, mask)
        }
    } else {
        // Fallback manual para CPUs sem BMI2
        let mut result = 0u64;
        let mut src = source;
        let mut msk = mask;
        
        while msk != 0 {
            let lsb = msk & msk.wrapping_neg();
            if (src & 1) != 0 {
                result |= lsb;
            }
            src >>= 1;
            msk &= msk - 1;
        }
        result
    }
}

// ============================================================================
// FUNÇÕES DE UTILIDADE PARA BITBOARDS
// ============================================================================

/// Itera sobre todos os bits setados em um bitboard de forma eficiente
pub struct BitboardIterator {
    bb: Bitboard,
}

impl BitboardIterator {
    #[inline(always)]
    pub fn new(bb: Bitboard) -> Self {
        Self { bb }
    }
}

impl Iterator for BitboardIterator {
    type Item = u8;

    #[inline(always)]
    fn next(&mut self) -> Option<Self::Item> {
        if self.bb == 0 {
            None
        } else {
            let square = trailing_zeros(self.bb) as u8;
            self.bb = reset_lsb(self.bb);
            Some(square)
        }
    }
}

/// Trait para operações de bitboard otimizadas
pub trait BitboardOps {
    fn iter_squares(self) -> BitboardIterator;
    fn popcount_fast(self) -> u32;
    fn lsb_fast(self) -> u32;
    fn msb_fast(self) -> u32;
    fn is_single_bit_fast(self) -> bool;
    fn isolate_lsb_fast(self) -> Bitboard;
    fn reset_lsb_fast(self) -> Bitboard;
}

impl BitboardOps for Bitboard {
    #[inline(always)]
    fn iter_squares(self) -> BitboardIterator {
        BitboardIterator::new(self)
    }

    #[inline(always)]
    fn popcount_fast(self) -> u32 {
        popcount(self)
    }

    #[inline(always)]
    fn lsb_fast(self) -> u32 {
        trailing_zeros(self)
    }

    #[inline(always)]
    fn msb_fast(self) -> u32 {
        63 - leading_zeros(self)
    }

    #[inline(always)]
    fn is_single_bit_fast(self) -> bool {
        is_single_bit(self)
    }

    #[inline(always)]
    fn isolate_lsb_fast(self) -> Bitboard {
        isolate_lsb(self)
    }

    #[inline(always)]
    fn reset_lsb_fast(self) -> Bitboard {
        reset_lsb(self)
    }
}

// ============================================================================
// FUNÇÕES DE DETECÇÃO DE FEATURES
// ============================================================================

/// Verifica se as extensões BMI1/BMI2 estão disponíveis
pub fn has_bmi_support() -> bool {
    #[cfg(target_arch = "x86_64")]
    {
        is_x86_feature_detected!("bmi1") && is_x86_feature_detected!("bmi2")
    }
    #[cfg(not(target_arch = "x86_64"))]
    {
        false
    }
}

/// Verifica se POPCNT está disponível
pub fn has_popcnt_support() -> bool {
    #[cfg(any(target_arch = "x86_64", target_arch = "x86"))]
    {
        is_x86_feature_detected!("popcnt")
    }
    #[cfg(not(any(target_arch = "x86_64", target_arch = "x86")))]
    {
        false
    }
}

/// Inicializa intrinsics com logs condicionais para performance máxima
#[cfg(debug_assertions)]
pub fn init_intrinsics() {
    println!("info string Intrinsics Support:");
    println!("info string - POPCNT: {}", has_popcnt_support());
    
    #[cfg(target_arch = "x86_64")]
    {
        println!("info string - BMI1: {}", is_x86_feature_detected!("bmi1"));
        println!("info string - BMI2: {}", is_x86_feature_detected!("bmi2"));
        println!("info string - LZCNT: {}", is_x86_feature_detected!("lzcnt"));
    }
    
    #[cfg(target_arch = "aarch64")]
    {
        println!("info string - AArch64 native intrinsics enabled");
    }
}

/// No-op em release builds para eliminar overhead
#[cfg(not(debug_assertions))]
pub fn init_intrinsics() {}

/// Índice de ocupação otimizado para magic bitboards usando PEXT quando disponível
#[inline(always)]
pub fn get_occupancy_index(occupancy: Bitboard, mask: Bitboard) -> u64 {
    #[cfg(target_arch = "x86_64")]
    {
        parallel_extract(occupancy & mask, mask)
    }
    #[cfg(not(target_arch = "x86_64"))]
    {
        // Fallback eficiente para outras arquiteturas
        let mut index = 0u64;
        let mut occ = occupancy & mask;
        let mut m = mask;
        let mut bit = 0;
        
        while m != 0 {
            if (occ & (m & m.wrapping_neg())) != 0 {
                index |= 1u64 << bit;
            }
            bit += 1;
            m &= m - 1;
        }
        index
    }
}