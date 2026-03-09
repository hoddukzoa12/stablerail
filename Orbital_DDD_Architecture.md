# Orbital Settlement Protocol — Domain-Driven Design

**Architecture Document v1.1**

---

## 1. Strategic Design: Bounded Contexts

시스템을 네 개의 Bounded Context로 분리한다. 각 Context는 독립적인 도메인 모델을 가지며, 명확한 인터페이스를 통해 상호작용한다.

**배포 전략: 단일 Anchor 프로그램 (Single Program)**

해커톤 MVP에서는 4개의 Bounded Context를 **하나의 Anchor 프로그램** 내 모듈로 구현한다. DDD의 논리적 분리는 Rust 모듈 수준에서 유지하되, 물리적 배포는 단일 Program ID로 통합한다.

```
┌─────────────────────────────────────────────────────────┐
│          Orbital Settlement Protocol (Single Program)    │
│          Program ID: ORBITAL_xxx                         │
│                                                         │
│  ┌──────────────┐  ┌──────────────┐  ┌──────────────┐  │
│  │   Orbital    │  │  Settlement  │  │   Policy     │  │
│  │   Core       │←─│  Context     │──│   Context    │  │
│  │   (module)   │  │  (module)    │  │  (module)    │  │
│  └──────┬───────┘  └──────────────┘  └──────────────┘  │
│         │                                               │
│  ┌──────┴───────┐                                       │
│  │  Liquidity   │                                       │
│  │  (module)    │                                       │
│  └──────────────┘                                       │
└─────────────────────────────────────────────────────────┘
```

**단일 프로그램 선택 이유:**

| 항목 | 4개 프로그램 분리 | 단일 프로그램 (선택) |
|------|-----------------|-------------------|
| Context 간 호출 | CPI (추가 CU 비용) | **모듈 함수 호출 (무비용)** |
| 계정 전달 | CPI마다 관련 계정 재전달 | **프로그램 내 직접 접근** |
| 배포/관리 | 4개 Program ID | **1개 Program ID** |
| PDA 권한 | 프로그램 간 PDA signer 필요 | **동일 프로그램이 모든 PDA 소유** |
| 개발 속도 | 느림 | **빠름 (해커톤 최적)** |
| DDD 원칙 | 물리적 분리 | **모듈 수준 논리적 분리** |

> **프로덕션 참고**: 프로덕션 환경에서는 독립적 업그레이드와 권한 분리를 위해 4개 프로그램으로 분리하는 것이 권장된다.

**Context 간 관계 (모듈 수준):**

- Settlement module → Orbital Core module: **Customer-Supplier** (Settlement이 swap 함수를 직접 호출, Core가 실행)
- Settlement module → Policy module: **Conformist** (Settlement이 Policy의 검증 함수를 직접 호출)
- Liquidity module → Orbital Core module: **Partnership** (LP와 Pool이 긴밀하게 협력)

---

## 2. Bounded Context 상세

### 2.1 Orbital Core Context

**책임**: Orbital AMM의 수학적 엔진. Sphere invariant, tick, swap 계산을 담당한다.

**Ubiquitous Language:**

| 용어 | 정의 |
|------|------|
| Pool | N개 스테이블코인의 Orbital AMM. Sphere invariant로 정의됨 |
| Sphere | reserve vector가 존재하는 n차원 구. center r⃗, radius r |
| Tick | $1 equal price point를 중심으로 한 spherical cap. 평면 x⃗·v⃗ = k로 정의 |
| Equal Price Point | 모든 reserve가 동일한 지점 q⃗. 모든 토큰 가격이 $1인 상태 |
| Reserve | Pool이 보유한 각 토큰의 수량 |
| Virtual Reserve | Tick boundary 덕분에 LP가 실제로 예치하지 않아도 되는 reserve |
| Interior Tick | 가격이 $1 근처일 때 활성 상태인 tick |
| Boundary Tick | 가격이 벗어나 경계에 고정된 tick |
| Torus Invariant | Interior + Boundary tick을 consolidate한 거래 계산용 invariant |

```
programs/orbital/src/contexts/core/
├── mod.rs
├── domain/
│   ├── mod.rs
│   ├── aggregates/
│   │   └── pool.rs             # Pool Aggregate Root
│   ├── entities/
│   │   └── tick.rs             # Tick Entity
│   ├── value_objects/
│   │   ├── sphere.rs           # Sphere (r, n, center)
│   │   ├── reserve.rs          # Reserve state vector
│   │   ├── price.rs            # Token price (r-xⱼ)/(r-xᵢ)
│   │   ├── tick_bound.rs       # k value (plane constant)
│   │   └── fixed_point.rs      # Q64.64 fixed-point number
│   ├── events/
│   │   ├── pool_created.rs
│   │   ├── swap_executed.rs
│   │   └── tick_crossed.rs
│   └── services/
│       ├── swap_calculator.rs  # Torus invariant swap 계산
│       ├── newton_solver.rs    # Newton's method for invariant
│       └── tick_consolidator.rs # Interior/Boundary tick consolidation
├── instructions/
│   ├── initialize_pool.rs      # Pool 생성 instruction
│   └── execute_swap.rs         # Swap 실행 instruction
└── accounts.rs                 # Solana account 구조 (Anchor)
```

**Aggregate: Pool**

Pool은 이 Context의 Aggregate Root다. 모든 상태 변경은 Pool을 통해 이루어진다.

```rust
// domain/aggregates/pool.rs

pub struct Pool {
    // --- Identity ---
    pub pool_id: Pubkey,

    // --- Sphere Parameters ---
    pub sphere: Sphere,          // Value Object: r, n

    // --- Reserve State ---
    pub reserves: ReserveState,  // Value Object: [x₁, x₂, ..., xₙ]

    // --- Tick Registry ---
    pub ticks: Vec<TickId>,      // Entity references
    pub total_interior_liquidity: FixedPoint,  // consolidated interior L
    pub total_boundary_liquidity: FixedPoint,  // consolidated boundary L

    // --- Pool Config ---
    pub fee_rate: FixedPoint,    // basis points
    pub token_mints: Vec<Pubkey>,
    pub token_vaults: Vec<Pubkey>,

    // --- Invariant Cache ---
    pub alpha_cache: FixedPoint,  // α = x⃗·v⃗ (running sum)
    pub w_norm_cache: FixedPoint, // ||w⃗||² (running)
}

impl Pool {
    /// Pool 불변조건: sphere invariant 검증
    fn verify_invariant(&self) -> Result<()> {
        // ||r⃗ - x⃗||² = r²
        let lhs = self.reserves.distance_squared_from_center(&self.sphere);
        let rhs = self.sphere.radius_squared();
        require!(lhs.approx_eq(rhs, EPSILON), OrbitalError::InvariantViolation);
        Ok(())
    }

    /// Swap 실행: 핵심 도메인 로직
    pub fn execute_swap(
        &mut self,
        token_in: usize,
        token_out: usize,
        amount_in: FixedPoint,
    ) -> Result<SwapResult> {
        // 1. Torus invariant로 amount_out 계산
        let swap_calc = SwapCalculator::new(&self.sphere);
        let result = swap_calc.compute(
            &self.reserves,
            token_in,
            token_out,
            amount_in,
            self.total_interior_liquidity,
            self.total_boundary_liquidity,
        )?;

        // 2. Tick crossing 확인
        if result.crosses_tick {
            self.handle_tick_crossing(&result)?;
        }

        // 3. Reserve 업데이트
        self.reserves.add(token_in, amount_in)?;
        self.reserves.sub(token_out, result.amount_out)?;

        // 4. Cache 업데이트
        self.update_caches()?;

        // 5. 불변조건 검증
        self.verify_invariant()?;

        // 6. Domain Event 발행
        emit!(SwapExecuted {
            pool: self.pool_id,
            token_in,
            token_out,
            amount_in,
            amount_out: result.amount_out,
            price: result.execution_price,
            slippage_bp: result.slippage_bp,
        });

        Ok(result)
    }
}
```

**Entity: Tick**

Tick은 고유 identity(k 값)를 가진 Entity다. LP에 의해 생성/소멸된다.

```rust
// domain/entities/tick.rs

pub struct Tick {
    // --- Identity ---
    pub tick_id: Pubkey,
    pub k: FixedPoint,            // plane constant (tick boundary)

    // --- State ---
    pub status: TickStatus,       // Interior | Boundary
    pub liquidity: FixedPoint,    // L (liquidity amount)
    pub sphere_radius: FixedPoint, // tick-local sphere radius s

    // --- Derived (from k) ---
    pub depeg_price: FixedPoint,  // p_depeg corresponding to this k
    pub x_min: FixedPoint,        // virtual reserve (minimum)
    pub x_max: FixedPoint,        // maximum token reserve
    pub capital_efficiency: FixedPoint, // x_base / (x_base - x_min)
}

#[derive(Clone, PartialEq)]
pub enum TickStatus {
    Interior,  // 가격이 tick range 안에 있음
    Boundary,  // 가격이 tick boundary에 도달, reserve 고정
}

impl Tick {
    /// Tick 생성 시 수학적 파라미터 계산
    pub fn new(k: FixedPoint, sphere: &Sphere) -> Result<Self> {
        let n = sphere.n;
        let r = sphere.radius;

        // k bounds 검증
        let k_min = r * (FixedPoint::sqrt(n) - FixedPoint::one());
        let k_max = r * (n - FixedPoint::one()) / FixedPoint::sqrt(n);
        require!(k > k_min && k < k_max, OrbitalError::InvalidTickBound);

        // x_min 계산 (virtual reserve)
        let x_min = Self::compute_x_min(k, r, n)?;

        // depeg price 계산
        let depeg_price = Self::compute_depeg_price(k, r, n)?;

        // capital efficiency 계산
        let x_base = r * (FixedPoint::one() - FixedPoint::one() / FixedPoint::sqrt(n));
        let capital_efficiency = x_base / (x_base - x_min);

        // boundary sphere radius
        let s = FixedPoint::sqrt(r * r - (k - r * FixedPoint::sqrt(n)).squared());

        Ok(Self {
            tick_id: Pubkey::default(),
            k,
            status: TickStatus::Interior,
            liquidity: FixedPoint::zero(),
            sphere_radius: s,
            depeg_price,
            x_min,
            x_max: Self::compute_x_max(k, r, n)?,
            capital_efficiency,
        })
    }

    fn compute_x_min(k: FixedPoint, r: FixedPoint, n: FixedPoint) -> Result<FixedPoint> {
        // x_min = (k√n - √(k²n - n((n-1)r - k√n)²)) / n
        let sqrt_n = FixedPoint::sqrt(n);
        let inner = k * k * n - n * ((n - FixedPoint::one()) * r - k * sqrt_n).squared();
        require!(inner >= FixedPoint::zero(), OrbitalError::MathOverflow);
        Ok((k * sqrt_n - FixedPoint::sqrt(inner)) / n)
    }

    fn compute_depeg_price(k: FixedPoint, r: FixedPoint, n: FixedPoint) -> Result<FixedPoint> {
        // 역산: k → x_depeg, x_other → price ratio
        let sqrt_n = FixedPoint::sqrt(n);
        let inner = k * k * n - n * ((n - FixedPoint::one()) * r - k * sqrt_n).squared();
        let x_depeg = (k * sqrt_n + FixedPoint::sqrt(inner)) / n;
        let x_other = (k * sqrt_n - x_depeg) / (n - FixedPoint::one());
        Ok((r - x_depeg) / (r - x_other))
    }

    fn compute_x_max(k: FixedPoint, r: FixedPoint, n: FixedPoint) -> Result<FixedPoint> {
        let sqrt_n = FixedPoint::sqrt(n);
        let inner = k * k * n - n * ((n - FixedPoint::one()) * r - k * sqrt_n).squared();
        let raw = (k * sqrt_n + FixedPoint::sqrt(inner)) / n;
        Ok(FixedPoint::min(r, raw))
    }
}
```

**Value Objects**

```rust
// domain/value_objects/sphere.rs

/// Sphere: 불변. 생성 후 변경되지 않는다.
#[derive(Clone, Copy)]
pub struct Sphere {
    pub radius: FixedPoint,  // r
    pub n: u8,               // asset count
}

impl Sphere {
    pub fn center(&self) -> Vec<FixedPoint> {
        // r⃗ = (r, r, ..., r)
        vec![self.radius; self.n as usize]
    }

    pub fn equal_price_point(&self) -> FixedPoint {
        // q = r(1 - 1/√n)
        self.radius * (FixedPoint::one() - FixedPoint::one() / FixedPoint::sqrt(self.n.into()))
    }

    pub fn unit_vector_component(&self) -> FixedPoint {
        // v⃗ component = 1/√n
        FixedPoint::one() / FixedPoint::sqrt(self.n.into())
    }

    pub fn radius_squared(&self) -> FixedPoint {
        self.radius * self.radius
    }
}
```

```rust
// domain/value_objects/reserve.rs

/// Reserve: Pool의 현재 토큰 보유량. 변경 시 새 인스턴스 생성.
#[derive(Clone)]
pub struct ReserveState {
    pub amounts: Vec<FixedPoint>,  // [x₁, x₂, ..., xₙ]
}

impl ReserveState {
    /// α = x⃗·v⃗ = (1/√n) Σxᵢ
    pub fn alpha(&self, sphere: &Sphere) -> FixedPoint {
        let sum: FixedPoint = self.amounts.iter().sum();
        sum * sphere.unit_vector_component()
    }

    /// ||w⃗||² = r² - (α - r√n)²
    pub fn w_norm_squared(&self, sphere: &Sphere) -> FixedPoint {
        let alpha = self.alpha(sphere);
        let r_sqrt_n = sphere.radius * FixedPoint::sqrt(sphere.n.into());
        sphere.radius_squared() - (alpha - r_sqrt_n).squared()
    }

    /// ||r⃗ - x⃗||²
    pub fn distance_squared_from_center(&self, sphere: &Sphere) -> FixedPoint {
        self.amounts.iter()
            .map(|x| (sphere.radius - *x).squared())
            .sum()
    }

    /// token j의 가격 (token i 기준)
    pub fn price(&self, i: usize, j: usize, sphere: &Sphere) -> FixedPoint {
        // (r - xⱼ) / (r - xᵢ)
        (sphere.radius - self.amounts[j]) / (sphere.radius - self.amounts[i])
    }
}
```

**Domain Service: SwapCalculator**

```rust
// domain/services/swap_calculator.rs

pub struct SwapCalculator {
    sphere: Sphere,
}

pub struct SwapResult {
    pub amount_out: FixedPoint,
    pub execution_price: FixedPoint,
    pub slippage_bp: FixedPoint,
    pub crosses_tick: bool,
    pub crossed_ticks: Vec<TickCrossing>,
    pub fee: FixedPoint,
}

pub struct TickCrossing {
    pub tick_id: TickId,
    pub from_status: TickStatus,
    pub to_status: TickStatus,
    pub alpha_at_crossing: FixedPoint,
}

impl SwapCalculator {
    pub fn compute(
        &self,
        reserves: &ReserveState,
        token_in: usize,
        token_out: usize,
        amount_in: FixedPoint,
        l_interior: FixedPoint,
        l_boundary: FixedPoint,
    ) -> Result<SwapResult> {
        // 1. Torus invariant로 거래 계산
        //    consolidate된 interior sphere + boundary circle
        let torus = TorusInvariant::new(
            &self.sphere,
            l_interior,
            l_boundary,
        );

        // 2. 단일 tick 내 거래 시도
        let (amount_out, new_alpha) = torus.compute_trade(
            reserves,
            token_in,
            token_out,
            amount_in,
        )?;

        // 3. Tick crossing 감지
        let crossings = self.detect_tick_crossings(
            reserves.alpha(&self.sphere),
            new_alpha,
        )?;

        // 4. Tick crossing이 있으면 segmented trade
        if !crossings.is_empty() {
            return self.compute_segmented_trade(
                reserves, token_in, token_out, amount_in, crossings
            );
        }

        // 5. Slippage 계산
        let mid_price = reserves.price(token_in, token_out, &self.sphere);
        let exec_price = amount_in / amount_out;
        let slippage_bp = ((exec_price - mid_price) / mid_price) * FixedPoint::from(10_000);

        Ok(SwapResult {
            amount_out,
            execution_price: exec_price,
            slippage_bp,
            crosses_tick: false,
            crossed_ticks: vec![],
            fee: amount_in * self.sphere.fee_rate(),
        })
    }
}
```

**Domain Service: TorusInvariant (Paradigm 원문 기반)**

Torus invariant는 모든 interior tick을 하나의 n차원 sphere로, 모든 boundary tick을 하나의 (n-1)차원 sphere로 consolidate한 결과다. 기하학적으로 sphere를 circle 위로 회전시킨 torus(도넛) 형태.

```rust
// domain/services/torus_invariant.rs

/// Torus Invariant: interior sphere + boundary circle의 합성
///
/// 원문 공식:
/// r_int² = (α_total - k_bound - r_int√n)²
///        + (||w_total|| - √(r_bound² - (k_bound - r_bound√n)²))²
///
/// 여기서:
///   α_total = (1/√n) Σx_total_i   (parallel component)
///   ||w_total|| = √(Σx_total_i² - (1/n)(Σx_total_i)²)  (orthogonal component)
///   r_int = consolidated interior radius (= Σ r_a for all interior ticks)
///   r_bound = consolidated boundary radius (= Σ s_a for all boundary ticks)
///   k_bound = boundary tick의 plane constant
pub struct TorusInvariant {
    sphere: Sphere,
    r_int: FixedPoint,     // consolidated interior sphere radius
    r_bound: FixedPoint,   // consolidated boundary sphere radius (in n-1 subspace)
    k_bound: FixedPoint,   // boundary tick plane constant
}

impl TorusInvariant {
    pub fn new(
        sphere: &Sphere,
        l_interior: FixedPoint,
        l_boundary: FixedPoint,
    ) -> Self {
        Self {
            sphere: *sphere,
            r_int: l_interior,
            r_bound: l_boundary,
            k_bound: /* boundary tick의 k value */,
        }
    }

    /// Constant-time trade computation
    ///
    /// Running sums 최적화 (Paradigm 원문):
    ///   구현체는 Σxᵢ (reserve sum)과 Σxᵢ² (squared reserve sum)을
    ///   상시 유지하여, 개별 거래가 2개 자산만 변경하므로
    ///   n과 무관한 O(1) 시간에 invariant를 계산한다.
    ///
    /// 거래 시 업데이트:
    ///   sum_new = sum_old + d_i - d_j
    ///   sq_sum_new = sq_sum_old + (x_i+d_i)² - x_i² + (x_j-d_j)² - x_j²
    pub fn compute_trade(
        &self,
        reserves: &ReserveState,
        token_in: usize,
        token_out: usize,
        amount_in: FixedPoint,
    ) -> Result<(FixedPoint, FixedPoint)> {
        // 1. Running sums에서 α, ||w|| 계산
        let sum_x = reserves.running_sum;       // Σxᵢ (상시 유지)
        let sq_sum_x = reserves.running_sq_sum; // Σxᵢ² (상시 유지)

        let n = self.sphere.n as u64;
        let sqrt_n = FixedPoint::sqrt(n.into());

        // α = (1/√n) * Σxᵢ
        let alpha = sum_x / sqrt_n;

        // ||w||² = Σxᵢ² - (1/n)(Σxᵢ)²
        let w_norm_sq = sq_sum_x - sum_x * sum_x / FixedPoint::from(n);

        // 2. token_in에 amount_in 추가 후 새로운 x_j를 torus invariant로 풀기
        //    x_i_new = x_i + amount_in
        //    torus invariant에서 x_j_new를 구하면 amount_out = x_j - x_j_new
        let x_i_new = reserves.amounts[token_in] + amount_in;

        // 3. Newton's method로 quartic equation 풀기
        let x_j_new = NewtonSolver::solve_for_reserve(
            &self,
            reserves,
            token_in,
            token_out,
            x_i_new,
        )?;

        let amount_out = reserves.amounts[token_out] - x_j_new;

        // 4. 새로운 α 계산 (tick crossing 감지용)
        let new_sum = sum_x + amount_in - amount_out;
        let new_alpha = new_sum / sqrt_n;

        Ok((amount_out, new_alpha))
    }
}
```

**Domain Service: NewtonSolver (Quartic Equation)**

```rust
// domain/services/newton_solver.rs

/// Newton's Method Solver for Torus Invariant
///
/// Paradigm 원문: 거래 시 token_in 변경 후 token_out을 구하려면
/// torus invariant에서 파생된 4차 방정식(quartic)을 풀어야 한다.
/// Newton's method로 10회 이내 수렴. 실패 시 bisection fallback.
pub struct NewtonSolver {
    max_iterations: u32,
    epsilon: FixedPoint,        // 수렴 판정 기준 (1e-12)
    bisection_max_iter: u32,    // fallback bisection 최대 반복
}

impl NewtonSolver {
    pub fn new() -> Self {
        Self {
            max_iterations: 10,
            epsilon: FixedPoint::from_raw(1), // Q64.64에서 최소 단위
            bisection_max_iter: 64,
        }
    }

    /// Torus invariant에서 x_j를 구하는 Newton iteration
    ///
    /// f(x_j) = torus_lhs(x_j) - r_int² = 0 을 풀기
    ///
    /// Newton step: x_j_{k+1} = x_j_k - f(x_j_k) / f'(x_j_k)
    pub fn solve_for_reserve(
        torus: &TorusInvariant,
        reserves: &ReserveState,
        token_in: usize,
        token_out: usize,
        x_i_new: FixedPoint,
    ) -> Result<FixedPoint> {
        // 초기 추정: 현재 reserve에서 시작
        let mut x_j = reserves.amounts[token_out];

        for iter in 0..Self::MAX_ITERATIONS {
            // f(x_j) = torus invariant LHS - r_int² 계산
            let (f_val, f_deriv) = Self::evaluate_torus_and_derivative(
                torus, reserves, token_in, token_out, x_i_new, x_j
            )?;

            // 수렴 확인
            if f_val.abs() < Self::EPSILON {
                return Ok(x_j);
            }

            // Newton step
            require!(f_deriv.abs() > FixedPoint::zero(), OrbitalError::NewtonDivergence);
            x_j = x_j - f_val / f_deriv;

            // x_j bounds 확인 (0 ≤ x_j ≤ r)
            x_j = x_j.clamp(FixedPoint::zero(), torus.sphere.radius);
        }

        // Newton 미수렴 → Bisection fallback
        Self::bisection_fallback(torus, reserves, token_in, token_out, x_i_new)
    }

    /// Bisection method fallback
    /// x_j ∈ [0, current_x_j] 범위에서 이분탐색
    fn bisection_fallback(
        torus: &TorusInvariant,
        reserves: &ReserveState,
        token_in: usize,
        token_out: usize,
        x_i_new: FixedPoint,
    ) -> Result<FixedPoint> {
        let mut lo = FixedPoint::zero();
        let mut hi = reserves.amounts[token_out];

        for _ in 0..Self::BISECTION_MAX_ITER {
            let mid = (lo + hi) / FixedPoint::from(2);
            let (f_val, _) = Self::evaluate_torus_and_derivative(
                torus, reserves, token_in, token_out, x_i_new, mid
            )?;

            if f_val.abs() < Self::EPSILON {
                return Ok(mid);
            }

            if f_val > FixedPoint::zero() {
                lo = mid;
            } else {
                hi = mid;
            }
        }

        Err(OrbitalError::SolverDidNotConverge.into())
    }
}
```

**Domain Service: TickConsolidator**

```rust
// domain/services/tick_consolidator.rs

/// Tick Consolidation (Paradigm 원문)
///
/// Interior Tick Consolidation:
///   두 interior tick a, b가 동일한 α 영역에 있으면
///   r_c = r_a + r_b 로 합산. reserve는 비례 스케일링: x_a = (r_a/r_b) * x_b
///
/// Boundary Tick Consolidation:
///   두 boundary tick은 (n-1)차원 부분공간에서 활동.
///   trade vector가 Δ·v = 0 (v 방향 성분 없음)을 만족.
///   s_c = s_a + s_b 로 합산.
pub struct TickConsolidator;

impl TickConsolidator {
    /// 모든 interior tick을 하나의 sphere로 consolidate
    pub fn consolidate_interior(ticks: &[Tick]) -> ConsolidatedTick {
        let r_int: FixedPoint = ticks.iter()
            .filter(|t| t.status == TickStatus::Interior)
            .map(|t| t.liquidity)  // 각 tick의 radius contribution
            .sum();

        ConsolidatedTick {
            radius: r_int,
            tick_type: ConsolidatedType::Interior,
        }
    }

    /// 모든 boundary tick을 하나의 (n-1)차원 sphere로 consolidate
    pub fn consolidate_boundary(ticks: &[Tick]) -> ConsolidatedTick {
        let s_bound: FixedPoint = ticks.iter()
            .filter(|t| t.status == TickStatus::Boundary)
            .map(|t| t.sphere_radius)  // s = boundary subspace radius
            .sum();

        ConsolidatedTick {
            radius: s_bound,
            tick_type: ConsolidatedType::Boundary,
        }
    }
}

pub struct ConsolidatedTick {
    pub radius: FixedPoint,
    pub tick_type: ConsolidatedType,
}

pub enum ConsolidatedType {
    Interior,  // n-dimensional sphere
    Boundary,  // (n-1)-dimensional sphere in orthogonal subspace
}
```

**Trade Segmentation Algorithm (Paradigm 원문 기반)**

```rust
// domain/services/swap_calculator.rs (continued)

impl SwapCalculator {
    /// Trade Segmentation: tick crossing이 발생하는 대규모 거래 처리
    ///
    /// Paradigm 원문 알고리즘:
    /// 1. tick crossing 없다고 가정하고 최종 state 계산
    /// 2. 검증: k_bound_max ≤ α_int_norm ≤ k_int_min
    /// 3. 위반 시 → 정확한 crossing point까지의 거래량 계산
    /// 4. crossed tick의 상태 전환 (Interior ↔ Boundary)
    /// 5. 남은 거래량으로 반복
    fn compute_segmented_trade(
        &self,
        reserves: &ReserveState,
        token_in: usize,
        token_out: usize,
        total_amount_in: FixedPoint,
        crossings: Vec<TickCrossing>,
    ) -> Result<SwapResult> {
        let mut current_reserves = reserves.clone();
        let mut remaining = total_amount_in;
        let mut total_out = FixedPoint::zero();
        let mut all_crossings = Vec::new();

        for crossing in crossings {
            // Crossover trade 계산 (Paradigm 원문):
            // α_crossover = x⃗·v⃗ at crossing point
            // d_j_crossover = √n * (α_total - α_crossover) + d_i_crossover
            // 이 관계식은 이차방정식(quadratic)으로 on-chain에서 풀 수 있다
            let (d_in_to_cross, d_out_to_cross) = self.compute_crossover_trade(
                &current_reserves,
                token_in,
                token_out,
                &crossing,
            )?;

            // Crossing point까지 거래 적용
            current_reserves.add(token_in, d_in_to_cross)?;
            current_reserves.sub(token_out, d_out_to_cross)?;
            total_out = total_out + d_out_to_cross;
            remaining = remaining - d_in_to_cross;

            // Tick 상태 전환
            all_crossings.push(crossing);
        }

        // 남은 거래량을 새로운 tick 구성에서 실행
        if remaining > FixedPoint::zero() {
            let (final_out, _) = self.torus.compute_trade(
                &current_reserves,
                token_in,
                token_out,
                remaining,
            )?;
            total_out = total_out + final_out;
        }

        let exec_price = total_amount_in / total_out;
        let mid_price = reserves.price(token_in, token_out, &self.sphere);
        let slippage_bp = ((exec_price - mid_price) / mid_price) * FixedPoint::from(10_000);

        Ok(SwapResult {
            amount_out: total_out,
            execution_price: exec_price,
            slippage_bp,
            crosses_tick: true,
            crossed_ticks: all_crossings,
            fee: total_amount_in * self.sphere.fee_rate(),
        })
    }

    /// Crossover Trade 계산 (Paradigm 원문)
    ///
    /// tick boundary에 정확히 도달하는 거래량을 이차방정식으로 계산:
    ///   d_j = √n * (α_total - α_crossover) + d_i
    /// 이를 torus invariant에 대입하면 d_i에 대한 quadratic이 됨.
    /// on-chain에서 quadratic formula로 해석적 풀이 가능.
    fn compute_crossover_trade(
        &self,
        reserves: &ReserveState,
        token_in: usize,
        token_out: usize,
        crossing: &TickCrossing,
    ) -> Result<(FixedPoint, FixedPoint)> {
        let sqrt_n = FixedPoint::sqrt(self.sphere.n.into());
        let alpha_cross = crossing.alpha_at_crossing;
        let alpha_current = reserves.alpha(&self.sphere);

        // Quadratic coefficients 계산
        // a*d_i² + b*d_i + c = 0 형태로 정리
        // quadratic formula: d_i = (-b ± √(b²-4ac)) / 2a
        let (a, b, c) = self.compute_quadratic_coefficients(
            reserves, token_in, token_out, alpha_cross
        )?;

        let discriminant = b * b - FixedPoint::from(4) * a * c;
        require!(discriminant >= FixedPoint::zero(), OrbitalError::NoCrossoverSolution);

        let sqrt_disc = FixedPoint::sqrt(discriminant);
        // 물리적으로 유효한 양의 근 선택
        let d_i = (-b + sqrt_disc) / (FixedPoint::from(2) * a);

        // d_j 계산
        let new_sum = reserves.running_sum + d_i;
        let new_alpha = new_sum / sqrt_n;
        let d_j = sqrt_n * (new_alpha - alpha_cross) + d_i;

        Ok((d_i, d_j))
    }

    /// Tick Crossing 감지 (Paradigm 원문)
    ///
    /// Normalized interior projection: α_norm = (x⃗·v⃗) / r
    /// Crossing 발생 조건: α_norm이 인접 tick의 boundary를 넘을 때
    /// 검증: k_bound_max ≤ α_norm ≤ k_int_min
    fn detect_tick_crossings(
        &self,
        old_alpha: FixedPoint,
        new_alpha: FixedPoint,
    ) -> Result<Vec<TickCrossing>> {
        let mut crossings = Vec::new();

        // α가 증가/감소하는 방향으로 tick boundary 순회
        // 각 tick의 k 값과 비교하여 crossing 감지
        // ...

        Ok(crossings)
    }
}
```

**Running Sums 최적화 (ReserveState 보완)**

```rust
// domain/value_objects/reserve.rs (보완)

/// Reserve: Running sums 최적화 (Paradigm 원문)
///
/// 구현체는 Σxᵢ와 Σxᵢ²를 상시 유지한다.
/// 개별 거래는 2개 자산만 변경하므로 O(1) 업데이트:
///   sum_new = sum_old + d_in - d_out
///   sq_sum_new = sq_sum_old + (x_i+d_in)² - x_i² + (x_j-d_out)² - x_j²
#[derive(Clone)]
pub struct ReserveState {
    pub amounts: Vec<FixedPoint>,      // [x₁, x₂, ..., xₙ]
    pub running_sum: FixedPoint,       // Σxᵢ (상시 유지)
    pub running_sq_sum: FixedPoint,    // Σxᵢ² (상시 유지)
}

impl ReserveState {
    pub fn new(amounts: Vec<FixedPoint>) -> Self {
        let running_sum = amounts.iter().copied().sum();
        let running_sq_sum = amounts.iter().map(|x| *x * *x).sum();
        Self { amounts, running_sum, running_sq_sum }
    }

    /// O(1) reserve 업데이트: token_in 증가, token_out 감소
    pub fn apply_trade(
        &mut self,
        token_in: usize,
        amount_in: FixedPoint,
        token_out: usize,
        amount_out: FixedPoint,
    ) {
        let old_i = self.amounts[token_in];
        let old_j = self.amounts[token_out];

        self.amounts[token_in] = old_i + amount_in;
        self.amounts[token_out] = old_j - amount_out;

        // Running sum O(1) 업데이트
        self.running_sum = self.running_sum + amount_in - amount_out;

        // Running squared sum O(1) 업데이트
        let new_i = self.amounts[token_in];
        let new_j = self.amounts[token_out];
        self.running_sq_sum = self.running_sq_sum
            + (new_i * new_i - old_i * old_i)
            + (new_j * new_j - old_j * old_j);
    }

    /// α = (1/√n) * Σxᵢ — running_sum으로 O(1) 계산
    pub fn alpha(&self, sphere: &Sphere) -> FixedPoint {
        self.running_sum * sphere.unit_vector_component()
    }

    /// ||w||² = Σxᵢ² - (1/n)(Σxᵢ)² — running sums로 O(1) 계산
    pub fn w_norm_squared(&self, sphere: &Sphere) -> FixedPoint {
        let n = FixedPoint::from(sphere.n as u64);
        self.running_sq_sum - self.running_sum * self.running_sum / n
    }

    /// ||r⃗ - x⃗||² (invariant 검증용)
    pub fn distance_squared_from_center(&self, sphere: &Sphere) -> FixedPoint {
        self.amounts.iter()
            .map(|x| (sphere.radius - *x).squared())
            .sum()
    }

    /// token j의 가격 (token i 기준)
    pub fn price(&self, i: usize, j: usize, sphere: &Sphere) -> FixedPoint {
        (sphere.radius - self.amounts[j]) / (sphere.radius - self.amounts[i])
    }
}
```

---

### 2.2 Liquidity Context

**책임**: LP position 관리, 유동성 예치/인출.

**Ubiquitous Language:**

| 용어 | 정의 |
|------|------|
| Position | LP의 유동성 포지션. 특정 Tick에 대한 자본 제공 |
| Deposit | LP가 Pool에 자산을 예치하는 행위 |
| Withdrawal | LP가 Pool에서 자산을 인출하는 행위 |
| Fee Share | Position이 수취한 누적 수수료 |

```
programs/orbital/src/contexts/liquidity/
├── mod.rs
├── domain/
│   ├── aggregates/
│   │   └── position.rs           # LP Position Aggregate Root
│   ├── value_objects/
│   │   ├── deposit_amounts.rs    # 각 토큰별 예치량
│   │   └── fee_snapshot.rs       # Fee 누적 스냅샷
│   ├── events/
│   │   ├── liquidity_added.rs
│   │   ├── liquidity_removed.rs
│   │   └── fees_claimed.rs
│   └── services/
│       └── deposit_calculator.rs # 실제/virtual reserve 기반 예치량 계산
├── instructions/
│   ├── add_liquidity.rs
│   ├── remove_liquidity.rs
│   └── claim_fees.rs
└── accounts.rs
```

**Aggregate: Position**

```rust
// domain/aggregates/position.rs

pub struct Position {
    // --- Identity ---
    pub position_id: Pubkey,
    pub owner: Pubkey,

    // --- Pool Reference ---
    pub pool: Pubkey,

    // --- Tick Selection ---
    pub tick_k: FixedPoint,           // 선택한 tick의 k 값
    pub depeg_tolerance: FixedPoint,  // 이 position의 depeg 허용 범위

    // --- Liquidity ---
    pub liquidity: FixedPoint,        // L (제공한 유동성 양)
    pub deposited_amounts: DepositAmounts, // 실제 예치한 토큰량

    // --- Fee Tracking ---
    pub fee_snapshot: FeeSnapshot,    // 마지막 claim 시점의 fee 상태
    pub unclaimed_fees: Vec<FixedPoint>,

    // --- Metadata ---
    pub created_at: i64,
    pub last_updated: i64,
}

impl Position {
    /// LP 지정 예치량의 유효성 검증 및 liquidity 계산
    ///
    /// LP는 각 토큰별 예치량(amounts)을 자유롭게 지정한다.
    /// 시스템은 amounts로부터 새 radius를 계산하고,
    /// sphere invariant(checkInvariants)를 만족하는지 검증한다.
    /// (reference impl: agrawalx/orbital-pool Orbital.sol L336-407)
    ///
    /// 제약: amounts[i] > 0 for all i < n (모든 토큰 예치 필수)
    pub fn validate_and_calculate_liquidity(
        tick_k: FixedPoint,
        amounts: &[FixedPoint],
        sphere: &Sphere,
    ) -> Result<(FixedPoint, DepositAmounts)> {
        // 1. 모든 토큰 > 0 검증
        require!(amounts.len() == sphere.n as usize, OrbitalError::InvalidAssetCount);
        for amount in amounts.iter() {
            require!(amount.raw > 0, OrbitalError::InvalidAmount);
        }

        // 2. amounts로 새 radius 계산 (reserves[0] 기반, reference impl 패턴)
        let new_radius = Self::calculate_radius_from_reserves(amounts, tick_k, sphere)?;

        // 3. sphere invariant 검증
        sphere.check_invariants(tick_k, new_radius, amounts)?;

        // 4. liquidity = r² (reference impl: (newRadius * newRadius) >> 48)
        let liquidity = new_radius.checked_mul(new_radius)?;

        Ok((liquidity, DepositAmounts { amounts: amounts.to_vec() }))
    }

    /// Equal price point 기준 최소 예치량 가이드 (UI 힌트용)
    ///
    /// LP가 amounts를 직접 지정하기 전에, equal price point에서의
    /// 균등 예치량을 참고값으로 제공한다.
    pub fn suggest_equal_deposit(
        tick_k: FixedPoint,
        target_liquidity: FixedPoint,
        sphere: &Sphere,
    ) -> Result<DepositAmounts> {
        let x_base = sphere.equal_price_point();
        let x_min = Tick::compute_x_min(tick_k, sphere.radius, sphere.n.into())?;
        let per_token = (x_base - x_min) * target_liquidity;
        let amounts = vec![per_token; sphere.n as usize];
        Ok(DepositAmounts { amounts })
    }

    /// Capital efficiency 조회
    pub fn capital_efficiency(&self, sphere: &Sphere) -> FixedPoint {
        let x_base = sphere.equal_price_point();
        let x_min = Tick::compute_x_min(self.tick_k, sphere.radius, sphere.n.into())
            .unwrap_or(FixedPoint::zero());
        x_base / (x_base - x_min)
    }
}
```

---

### 2.3 Settlement Context

**책임**: 기관 사용자의 stablecoin settlement 워크플로우.

**Ubiquitous Language:**

| 용어 | 정의 |
|------|------|
| Settlement | 기관이 요청한 stablecoin 전환 거래 |
| Operator | Settlement을 실행할 수 있는 인가된 wallet |
| Settlement Request | Operator가 생성한 전환 요청 |
| Audit Entry | Settlement 실행 기록. 불변. |

```
programs/orbital/src/contexts/settlement/
├── mod.rs
├── domain/
│   ├── aggregates/
│   │   └── settlement.rs          # Settlement Aggregate Root
│   ├── entities/
│   │   └── audit_entry.rs         # Audit log entry
│   ├── value_objects/
│   │   ├── settlement_request.rs  # 전환 요청 파라미터
│   │   ├── settlement_result.rs   # 실행 결과
│   │   └── operator_role.rs       # Admin | Operator | Viewer
│   ├── events/
│   │   ├── settlement_requested.rs
│   │   ├── settlement_executed.rs
│   │   └── settlement_rejected.rs
│   └── services/
│       └── settlement_executor.rs # 정책 검증 → swap 실행 → audit 기록
├── instructions/
│   ├── execute_settlement.rs
│   └── query_audit.rs
└── accounts.rs
```

**Aggregate: Settlement**

```rust
// domain/aggregates/settlement.rs

pub struct Settlement {
    // --- Identity ---
    pub settlement_id: Pubkey,

    // --- Participants ---
    pub institution: Pubkey,    // 소속 기관 (Policy Context 참조)
    pub operator: Pubkey,       // 실행자

    // --- Request ---
    pub request: SettlementRequest,

    // --- Result ---
    pub status: SettlementStatus,
    pub result: Option<SettlementResult>,

    // --- Audit ---
    pub audit_entry: Option<AuditEntry>,

    // --- Timestamps ---
    pub requested_at: i64,
    pub executed_at: Option<i64>,
}

#[derive(Clone)]
pub enum SettlementStatus {
    Pending,
    Executed,
    Rejected { reason: RejectReason },
}

#[derive(Clone)]
pub enum RejectReason {
    PolicyViolation(String),
    InsufficientLiquidity,
    SlippageExceeded,
    UnauthorizedOperator,
}

impl Settlement {
    /// Settlement 실행: Policy 검증 → Swap → Audit
    pub fn execute(
        &mut self,
        policy: &Policy,
        pool: &mut Pool,    // Orbital Core module의 Pool (같은 프로그램 내 직접 접근)
        clock: &Clock,
    ) -> Result<SettlementResult> {
        // 1. 상태 검증
        require!(self.status == SettlementStatus::Pending, SettlementError::AlreadyProcessed);

        // 2. Policy 검증 (Policy Context에 위임)
        policy.validate_settlement(&self.request, &self.operator)?;

        // 3. Swap 실행 (Core module 함수 직접 호출)
        let swap_result = pool.execute_swap(
            self.request.token_in_index,
            self.request.token_out_index,
            self.request.amount_in,
        )?;

        // 4. Slippage 검증
        require!(
            swap_result.slippage_bp <= self.request.max_slippage_bp,
            SettlementError::SlippageExceeded
        );

        // 5. Result 생성
        let result = SettlementResult {
            amount_in: self.request.amount_in,
            amount_out: swap_result.amount_out,
            execution_price: swap_result.execution_price,
            slippage_bp: swap_result.slippage_bp,
            fee: swap_result.fee,
        };

        // 6. Audit Entry 생성 (불변)
        let audit = AuditEntry {
            settlement_id: self.settlement_id,
            institution: self.institution,
            operator: self.operator,
            request: self.request.clone(),
            result: result.clone(),
            policy_id: policy.policy_id,
            timestamp: clock.unix_timestamp,
        };

        // 7. 상태 전이
        self.status = SettlementStatus::Executed;
        self.result = Some(result.clone());
        self.audit_entry = Some(audit.clone());
        self.executed_at = Some(clock.unix_timestamp);

        // 8. Domain Event
        emit!(SettlementExecuted {
            settlement_id: self.settlement_id,
            institution: self.institution,
            amount_in: result.amount_in,
            amount_out: result.amount_out,
            slippage_bp: result.slippage_bp,
        });

        Ok(result)
    }
}
```

**Value Object: SettlementRequest**

```rust
// domain/value_objects/settlement_request.rs

/// 불변. 생성 후 변경되지 않는다.
#[derive(Clone)]
pub struct SettlementRequest {
    pub token_in: Pubkey,       // SPL token mint
    pub token_out: Pubkey,      // SPL token mint
    pub token_in_index: usize,  // Pool 내 인덱스
    pub token_out_index: usize,
    pub amount_in: FixedPoint,
    pub max_slippage_bp: FixedPoint,  // 허용 최대 슬리피지
}

impl SettlementRequest {
    pub fn validate(&self) -> Result<()> {
        require!(self.token_in != self.token_out, SettlementError::SameToken);
        require!(self.amount_in > FixedPoint::zero(), SettlementError::ZeroAmount);
        require!(self.max_slippage_bp >= FixedPoint::zero(), SettlementError::InvalidSlippage);
        Ok(())
    }
}
```

**Entity: AuditEntry**

```rust
// domain/entities/audit_entry.rs

/// Audit entry. 생성 후 절대 수정/삭제되지 않는다.
/// on-chain에 영구 기록.
pub struct AuditEntry {
    pub settlement_id: Pubkey,
    pub institution: Pubkey,
    pub operator: Pubkey,
    pub request: SettlementRequest,
    pub result: SettlementResult,
    pub policy_id: Pubkey,
    pub timestamp: i64,
}
```

---

### 2.4 Policy Context

**책임**: 기관의 접근 제어, 거래 정책, 권한 관리.

**Ubiquitous Language:**

| 용어 | 정의 |
|------|------|
| Policy | 기관이 설정한 settlement 규칙의 집합 |
| Institution | Policy를 소유하는 기관 |
| Allowlist | Settlement을 실행할 수 있는 wallet 목록 |
| Role | Admin, Operator, Viewer 중 하나 |
| Limit | 거래 금액 또는 빈도 제한 |

```
programs/orbital/src/contexts/policy/
├── mod.rs
├── domain/
│   ├── aggregates/
│   │   └── policy.rs              # Policy Aggregate Root
│   ├── entities/
│   │   └── allowlist_entry.rs     # Allowlist member
│   ├── value_objects/
│   │   ├── role.rs                # Admin | Operator | Viewer
│   │   ├── token_whitelist.rs     # 허용 토큰 목록
│   │   └── limits.rs              # 거래 한도
│   ├── events/
│   │   ├── policy_created.rs
│   │   ├── policy_updated.rs
│   │   ├── member_added.rs
│   │   └── member_removed.rs
│   └── services/
│       └── policy_validator.rs    # Settlement 요청의 정책 준수 검증
├── instructions/
│   ├── create_policy.rs
│   ├── update_policy.rs
│   └── manage_allowlist.rs
└── accounts.rs
```

**Aggregate: Policy**

```rust
// domain/aggregates/policy.rs

pub struct Policy {
    // --- Identity ---
    pub policy_id: Pubkey,

    // --- Ownership ---
    pub institution: Pubkey,
    pub admin: Pubkey,

    // --- Rules ---
    pub allowed_tokens: TokenWhitelist,
    pub per_tx_limit: FixedPoint,          // 단일 거래 최대 금액
    pub daily_limit: FixedPoint,           // 일일 총 거래 한도
    pub daily_volume_used: FixedPoint,     // 오늘 사용된 volume
    pub last_reset_date: i64,              // daily limit 리셋 시점

    // --- Access Control ---
    pub allowlist: Vec<AllowlistEntry>,

    // --- Status ---
    pub is_active: bool,
    pub created_at: i64,
    pub updated_at: i64,
}

impl Policy {
    /// Settlement 요청의 정책 준수 여부 검증
    pub fn validate_settlement(
        &self,
        request: &SettlementRequest,
        operator: &Pubkey,
    ) -> Result<()> {
        // 1. Policy 활성 상태 확인
        require!(self.is_active, PolicyError::PolicyInactive);

        // 2. Operator 권한 확인
        let entry = self.find_member(operator)?;
        require!(
            entry.role == Role::Operator || entry.role == Role::Admin,
            PolicyError::InsufficientRole
        );

        // 3. Token whitelist 확인
        require!(
            self.allowed_tokens.contains(&request.token_in),
            PolicyError::TokenNotAllowed
        );
        require!(
            self.allowed_tokens.contains(&request.token_out),
            PolicyError::TokenNotAllowed
        );

        // 4. Per-transaction limit
        require!(
            request.amount_in <= self.per_tx_limit,
            PolicyError::ExceedsTransactionLimit
        );

        // 5. Daily limit
        let new_daily = self.daily_volume_used + request.amount_in;
        require!(
            new_daily <= self.daily_limit,
            PolicyError::ExceedsDailyLimit
        );

        Ok(())
    }

    /// Allowlist에 멤버 추가 (Admin만 가능)
    pub fn add_member(
        &mut self,
        caller: &Pubkey,
        wallet: Pubkey,
        role: Role,
    ) -> Result<()> {
        require!(*caller == self.admin, PolicyError::NotAdmin);
        require!(!self.has_member(&wallet), PolicyError::AlreadyMember);

        self.allowlist.push(AllowlistEntry {
            wallet,
            role,
            added_at: Clock::get()?.unix_timestamp,
        });

        emit!(MemberAdded {
            policy: self.policy_id,
            wallet,
            role,
        });

        Ok(())
    }
}
```

---

## 3. Context Map: 상호작용

단일 프로그램 내에서 모듈 간 함수 호출로 상호작용한다. CPI 오버헤드 없이 직접 호출.

```
                    ┌─────────────────┐
                    │  Policy Module   │
                    │                 │
                    │  Policy ──────────── validate_settlement()
                    │                 │          │
                    └────────┬────────┘          │
                             │ (fn call)         │
                    ┌────────▼────────┐          │
                    │   Settlement    │          │
                    │   Module        │          │
                    │                 │          │
   User Request ──→│  Settlement ────────── execute()
                    │                 │          │
                    └────────┬────────┘          │
                             │ (fn call)         │
                    ┌────────▼────────┐          │
                    │  Orbital Core   │          │
                    │  Module         │          │
                    │                 │          │
                    │  Pool.execute_swap() ──────┘
                    │                 │
                    └────────┬────────┘
                             │ (fn call)
                    ┌────────▼────────┐
                    │  Liquidity      │
                    │  Module         │
                    │                 │
                    │  Position       │
                    │  (LP only)      │
                    └─────────────────┘
```

**Module Boundary: Settlement → Orbital Core**

Settlement module은 Orbital Core의 내부 수학 모델을 직접 알 필요 없다. 모듈 간 공개 인터페이스(pub fn)를 통해 swap 요청만 전달하고 결과를 받는다. CPI 대신 같은 프로그램 내 함수 호출.

```rust
// contexts/settlement/services/settlement_executor.rs

use crate::contexts::core::domain::aggregates::pool::Pool;
use crate::contexts::policy::domain::aggregates::policy::Policy;

/// Settlement 실행: Policy 검증 → Core swap 함수 직접 호출 → Audit 기록
/// CPI 없이 같은 프로그램 내 모듈 함수 호출로 처리
pub fn execute_settlement(
    settlement: &mut Settlement,
    policy: &Policy,
    pool: &mut Pool,
    clock: &Clock,
) -> Result<SettlementResult> {
    // 1. Policy module의 검증 함수 직접 호출
    policy.validate_settlement(&settlement.request, &settlement.operator)?;

    // 2. Core module의 swap 함수 직접 호출 (CPI 불필요)
    let swap_result = pool.execute_swap(
        settlement.request.token_in_index,
        settlement.request.token_out_index,
        settlement.request.amount_in,
    )?;

    // 3. Slippage 검증 및 결과 생성
    require!(
        swap_result.slippage_bp <= settlement.request.max_slippage_bp,
        SettlementError::SlippageExceeded
    );

    Ok(SettlementResult::from(swap_result))
}
```

---

## 4. Solana Account Model Mapping

DDD의 Aggregate/Entity를 Solana의 Account로 매핑한다. 모든 PDA는 **단일 Program ID** 아래에 생성된다.

```
┌─────────────────────────────────────────────────────┐
│ Orbital Program (program_id: ORBITAL_xxx)            │
│ ═══════════════════════════════════════════════════  │
│                                                     │
│ ┌─ Core Context ──────────────────────────────────┐ │
│ │                                                 │ │
│ │  Pool Account (PDA)                             │ │
│ │  ├── seeds: ["pool", pool_id]                   │ │
│ │  ├── sphere params (r, n)                       │ │
│ │  ├── reserves [x₁, x₂, x₃]                    │ │
│ │  ├── running_sum, running_sq_sum                │ │
│ │  ├── fee_rate                                   │ │
│ │  └── token_mints[], token_vaults[]              │ │
│ │                                                 │ │
│ │  Tick Account (PDA) — per tick                  │ │
│ │  ├── seeds: ["tick", pool_id, k_bytes]          │ │
│ │  ├── k, status, liquidity, sphere_radius        │ │
│ │  └── derived params (x_min, depeg_price, etc)   │ │
│ │                                                 │ │
│ │  Token Vault Accounts (PDA) — per token         │ │
│ │  └── seeds: ["vault", pool_id, mint]            │ │
│ └─────────────────────────────────────────────────┘ │
│                                                     │
│ ┌─ Liquidity Context ─────────────────────────────┐ │
│ │                                                 │ │
│ │  Position Account (PDA) — per LP position       │ │
│ │  ├── seeds: ["position", owner, pool_id, k]     │ │
│ │  ├── owner, pool, tick_k, liquidity             │ │
│ │  ├── deposited_amounts[]                        │ │
│ │  └── fee tracking                               │ │
│ └─────────────────────────────────────────────────┘ │
│                                                     │
│ ┌─ Settlement Context ────────────────────────────┐ │
│ │                                                 │ │
│ │  Settlement Account (PDA) — per settlement      │ │
│ │  ├── seeds: ["settlement", institution, nonce]  │ │
│ │  ├── request params, status, result             │ │
│ │  └── audit_entry                                │ │
│ └─────────────────────────────────────────────────┘ │
│                                                     │
│ ┌─ Policy Context ────────────────────────────────┐ │
│ │                                                 │ │
│ │  Policy Account (PDA) — per institution         │ │
│ │  ├── seeds: ["policy", institution]             │ │
│ │  ├── admin, allowed_tokens[], limits            │ │
│ │  └── daily_volume_used                          │ │
│ │                                                 │ │
│ │  Allowlist Entry Account (PDA) — per member     │ │
│ │  ├── seeds: ["member", policy_id, wallet]       │ │
│ │  ├── role, added_at                             │ │
│ └─────────────────────────────────────────────────┘ │
└─────────────────────────────────────────────────────┘
```

> **장점**: 모든 PDA가 동일한 program_id를 owner로 가지므로, Settlement instruction에서 Pool 계정을 직접 수정할 수 있다. CPI 없이 `&mut pool` 참조로 swap 로직 실행 가능.

---

## 5. Computation Architecture: Off-chain / On-chain 분리

### 5.1 설계 원칙

Solana의 CU(Compute Unit) 제한 하에서 Orbital의 수학 연산을 효율적으로 처리하기 위해, **off-chain 계산 + on-chain 검증** 패턴을 채택한다. 이는 Jupiter, Orca(Whirlpool), Meteora 등 Solana DeFi 프로토콜의 표준 패턴이다.

**Reference impl(agrawalx/orbital-pool)과의 차이점**: Arbitrum Stylus는 Rust→WASM을 on-chain에서 실행할 수 있어 무거운 수학(Newton's method 등)도 on-chain에서 처리한다. Solana는 WASM 런타임이 없으므로, 동일한 연산을 off-chain으로 분리한다.

```
┌─────────────────────────────────┐     ┌──────────────────────────────────┐
│  Off-chain (TypeScript SDK)     │     │  On-chain (Anchor Program)       │
│                                 │     │                                  │
│  • radius 계산                  │     │  • amounts[i] > 0 검증           │
│  • tick params (k, r) 도출      │ ──→ │  • Σ(r - xᵢ)² ≈ r² 검증 (O(n)) │
│  • liquidity = r² 계산          │ IX  │  • SPL token transfer            │
│  • invariant 사전 검증          │     │  • Position PDA 생성/업데이트     │
│  • Newton's method (swap quote) │     │  • tick crossing 감지            │
│  • 최적 amounts 추천 (UI)       │     │  • reserve 업데이트              │
└─────────────────────────────────┘     └──────────────────────────────────┘
```

### 5.2 연산 분류

| 연산 | 위치 | CU 비용 | 근거 |
|------|------|---------|------|
| `checkInvariants` (Σ(r-xᵢ)²≈r²) | **on-chain** | ~5K (n=3) | 단순 산술, 검증 필수 |
| `calculateTickParams` (r, k) | **off-chain** | — | reserves[0] 기반 계산 |
| Newton's method (torus solver) | **off-chain** | — | 반복 연산, CU 소모 큼 |
| `suggestEqualDeposit` (UI hint) | **off-chain** | — | (x_base - x_min) × L |
| swap amount_out 계산 | **off-chain** | — | Newton iteration 필요 |
| swap amount_out 검증 | **on-chain** | ~8K (n=3) | invariant 재검증으로 충분 |
| LP shares 계산 | **on-chain** | ~2K | r² 기반, 단순 곱셈 |

### 5.3 TypeScript SDK 구조

MVP에서는 Q64.64를 `BigInt`로 구현하여 on-chain Rust i128과 정밀도를 동일하게 유지한다. Newton's method 등 복잡한 수학이 필요한 Post-MVP 단계에서 Rust→WASM 컴파일을 고려한다.

```typescript
// sdk/orbital-math.ts — Q64.64 over BigInt

const FRAC_BITS = 64n;
const SCALE = 1n << FRAC_BITS;

/** Sphere invariant 검증: Σ(r - xᵢ)² ≈ r² (0.1% tolerance) */
function checkInvariants(r: bigint, amounts: bigint[]): boolean {
  const rSquared = (r * r) >> FRAC_BITS;
  const tolerance = rSquared / 1000n;
  let sumDiffSq = 0n;
  for (const x of amounts) {
    const diff = r - x;
    sumDiffSq += (diff * diff) >> FRAC_BITS;
  }
  return sumDiffSq >= rSquared - tolerance
      && sumDiffSq <= rSquared + tolerance;
}

/** Tick params 계산: reserves[0]로부터 radius 도출 */
function calculateTickParams(
  p: bigint,              // plane constant
  reserve0: bigint,       // reserves[0] (Q64.64)
  n: number,              // asset count
): { k: bigint; r: bigint } {
  // r = reserve0 / (1 - 1/√n)
  // k = Σreserves / √n
  // ... (reference impl 패턴)
}

/** Equal price point 기준 균등 예치량 추천 (UI hint) */
function suggestEqualDeposit(
  tickK: bigint,
  targetLiquidity: bigint,
  sphere: { radius: bigint; n: number },
): bigint[] {
  const xBase = equalPricePoint(sphere);
  const xMin = computeXMin(tickK, sphere.radius, sphere.n);
  const perToken = ((xBase - xMin) * targetLiquidity) >> FRAC_BITS;
  return Array(sphere.n).fill(perToken);
}
```

### 5.4 Instruction별 off-chain/on-chain 역할

| Instruction | Off-chain (SDK) | On-chain (Program) |
|---|---|---|
| `add_liquidity` | amounts 결정, radius·liquidity 계산, invariant 사전검증 | amounts>0, invariant 재검증, SPL transfer, Position 생성 |
| `remove_liquidity` | 반환량 계산, LP shares 비율 산출 | position ownership, balance 확인, SPL transfer |
| `execute_swap` | Newton's method로 amount_out 계산, 최적 tick 경로 탐색 | invariant 재검증, slippage 확인, reserve 업데이트, SPL transfer |
| `execute_settlement` | swap quote 조회, policy 사전검증 | policy enforcement, swap 실행, audit entry 생성 |

### 5.5 Post-MVP: Rust → WASM 마이그레이션 경로

MVP의 TypeScript SDK가 한계에 도달하는 시점 (n>5 또는 Newton iteration >10회):

```
MVP (현재)                          Post-MVP
───────────                        ─────────
TypeScript BigInt SDK       →      Rust math crate → wasm-pack → @orbital/math-wasm
                                   • on-chain과 100% 동일한 연산 보장
                                   • wasm-bindgen으로 TS 인터페이스 자동 생성
                                   • math/ 디렉토리의 Rust 코드를 dual-target 빌드:
                                     - target: bpf (Solana on-chain)
                                     - target: wasm32 (브라우저/Node off-chain)
```

---

## 6. Domain Events Flow

Settlement 실행의 전체 이벤트 흐름 (단일 프로그램 내 모듈 간 함수 호출):

```
Operator가 execute_settlement instruction 호출
    │
    ▼
SettlementRequested {
    settlement_id, institution, operator,
    token_in, token_out, amount_in
}
    │
    ├── Policy module: policy.validate_settlement() (fn call)
    │   ├── 성공 → continue
    │   └── 실패 → SettlementRejected { reason }
    │
    ▼
Core module: pool.execute_swap() (fn call, CPI 불필요)
    │
    ├── SwapExecuted {
    │       pool, token_in, token_out,
    │       amount_in, amount_out, slippage_bp
    │   }
    │
    ├── (tick crossing 발생 시)
    │   └── TickCrossed {
    │           tick_id, from: Interior, to: Boundary
    │       }
    │
    ▼
SettlementExecuted {
    settlement_id, institution,
    amount_in, amount_out, slippage_bp,
    policy_id, timestamp
}
    │
    ▼
AuditEntry 생성 (on-chain, 불변)
```

> **참고**: 모든 이벤트는 하나의 transaction 내에서 emit된다. 단일 프로그램이므로 atomic하게 처리.

---

## 6. 디렉토리 전체 구조

단일 Anchor 프로그램 내에서 4개 Bounded Context를 모듈로 분리한다.

```
orbital-settlement-protocol/
│
├── programs/
│   └── orbital/                    # 단일 Anchor 프로그램
│       ├── src/
│       │   ├── lib.rs              # #[program] entrypoint + instruction dispatch
│       │   ├── errors.rs           # 전체 에러 타입
│       │   │
│       │   ├── math/               # 공유 수학 라이브러리
│       │   │   ├── mod.rs
│       │   │   ├── q64_64.rs       # Q64.64 fixed-point arithmetic
│       │   │   ├── sphere.rs       # Sphere invariant
│       │   │   ├── torus.rs        # Torus invariant
│       │   │   └── newton.rs       # Newton solver
│       │   │
│       │   ├── state/              # 공유 account 구조체 (Anchor accounts)
│       │   │   ├── mod.rs
│       │   │   ├── pool.rs         # Pool account
│       │   │   ├── tick.rs         # Tick account
│       │   │   ├── position.rs     # LP Position account
│       │   │   ├── policy.rs       # Policy account
│       │   │   ├── allowlist.rs    # Allowlist Entry account
│       │   │   ├── settlement.rs   # Settlement account
│       │   │   └── audit.rs        # Audit Entry account
│       │   │
│       │   └── instructions/       # Bounded Context별 instructions
│       │       ├── mod.rs
│       │       │
│       │       ├── core/           # Orbital Core Context
│       │       │   ├── mod.rs
│       │       │   ├── initialize_pool.rs
│       │       │   └── execute_swap.rs
│       │       │
│       │       ├── liquidity/      # Liquidity Context
│       │       │   ├── mod.rs
│       │       │   ├── add_liquidity.rs
│       │       │   ├── remove_liquidity.rs
│       │       │   └── claim_fees.rs
│       │       │
│       │       ├── settlement/     # Settlement Context
│       │       │   ├── mod.rs
│       │       │   ├── execute_settlement.rs
│       │       │   └── query_audit.rs
│       │       │
│       │       └── policy/         # Policy Context
│       │           ├── mod.rs
│       │           ├── create_policy.rs
│       │           ├── update_policy.rs
│       │           └── manage_allowlist.rs
│       │
│       ├── Cargo.toml
│       └── Xargo.toml
│
├── app/                            # Frontend (Next.js via create-solana-dapp)
│   ├── src/
│   │   ├── app/                    # Next.js App Router
│   │   │   ├── page.tsx            # Swap UI (메인)
│   │   │   ├── settlement/         # Operator view
│   │   │   ├── admin/              # Policy admin view
│   │   │   └── dashboard/          # Pool stats view
│   │   ├── hooks/
│   │   │   ├── useOrbitalPool.ts
│   │   │   ├── useSettlement.ts
│   │   │   └── usePolicy.ts
│   │   ├── components/             # Shared UI components
│   │   └── lib/
│   │       ├── idl/                # Anchor IDL (auto-generated)
│   │       └── orbital-math/       # Off-chain math SDK (TypeScript BigInt)
│   │           ├── q64-64.ts       # Q64.64 fixed-point (BigInt, on-chain i128 동일)
│   │           ├── sphere.ts       # Sphere invariant 검증 (checkInvariants)
│   │           ├── tick-params.ts  # calculateTickParams (radius, k 계산)
│   │           ├── deposit.ts      # suggestEqualDeposit (UI hint)
│   │           └── index.ts        # barrel export
│   ├── package.json
│   ├── next.config.ts
│   └── tsconfig.json
│
├── scripts/
│   ├── bootstrap.ts                # Mock token mint + 초기 LP 예치
│   └── demo.ts                     # 데모 시나리오 자동 실행
│
├── tests/
│   ├── integration/
│   │   ├── full_settlement_flow.rs
│   │   └── policy_enforcement.rs
│   └── unit/
│       ├── sphere_math.rs
│       └── tick_operations.rs
│
├── Anchor.toml
├── Cargo.toml (workspace)
└── README.md
```

**lib.rs 구조 예시:**

```rust
// programs/orbital/src/lib.rs

use anchor_lang::prelude::*;

pub mod errors;
pub mod math;
pub mod state;
pub mod instructions;

use instructions::*;

declare_id!("ORBITAL_PROGRAM_ID_HERE");

#[program]
pub mod orbital {
    use super::*;

    // === Core Context ===
    pub fn initialize_pool(ctx: Context<InitializePool>, params: InitPoolParams) -> Result<()> {
        core::initialize_pool::handler(ctx, params)
    }

    pub fn execute_swap(ctx: Context<ExecuteSwap>, params: SwapParams) -> Result<()> {
        core::execute_swap::handler(ctx, params)
    }

    // === Liquidity Context ===
    pub fn add_liquidity(ctx: Context<AddLiquidity>, params: AddLiquidityParams) -> Result<()> {
        liquidity::add_liquidity::handler(ctx, params)
    }

    pub fn remove_liquidity(ctx: Context<RemoveLiquidity>, params: RemoveLiquidityParams) -> Result<()> {
        liquidity::remove_liquidity::handler(ctx, params)
    }

    // === Policy Context ===
    pub fn create_policy(ctx: Context<CreatePolicy>, params: CreatePolicyParams) -> Result<()> {
        policy::create_policy::handler(ctx, params)
    }

    pub fn manage_allowlist(ctx: Context<ManageAllowlist>, params: AllowlistParams) -> Result<()> {
        policy::manage_allowlist::handler(ctx, params)
    }

    // === Settlement Context ===
    pub fn execute_settlement(ctx: Context<ExecuteSettlement>, params: SettlementParams) -> Result<()> {
        // Settlement instruction 내에서 policy 검증 + core swap 함수 직접 호출
        settlement::execute_settlement::handler(ctx, params)
    }
}
```
