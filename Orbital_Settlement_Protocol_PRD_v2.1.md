# PRD: Orbital Settlement Protocol on Solana

**StableHacks 2026 Submission**
**Version 2.1 — March 2026**

---

## 1. Executive Summary

Orbital Settlement Protocol은 Paradigm이 설계한 차세대 stablecoin AMM인 Orbital을 Solana에 최초로 구현하고, 그 위에 기관용 settlement layer를 결합한 프로토콜이다.

기존 Orbital 구현체는 EVM(Arbitrum Stylus)과 THORChain(CosmWasm)에만 존재한다. Solana의 고성능 실행 환경에서 Orbital의 n차원 concentrated liquidity를 구현함으로써, 기관이 실제로 사용할 수 있는 수준의 stablecoin clearing infrastructure를 제공한다.

**한 줄 요약**: Paradigm Orbital AMM의 Solana-native 구현 + 기관용 permissioned settlement layer

---

## 2. Problem Statement

### 2.1 Stablecoin Liquidity Fragmentation

달러형 스테이블코인의 수가 급증하고 있다. USDC, USDT, PYUSD, FDUSD, EURC, RLUSD 등이 이미 존재하며, 은행 발행 stablecoin, tokenized deposit, CBDC가 추가될 예정이다. 미국 상원은 GENIUS Act를 통과시켰고, JPMorgan·Bank of America·Citigroup은 공동 stablecoin 발행을 논의 중이다.

각 스테이블코인은 별도의 유동성 풀을 가진다. N개의 스테이블코인 간 거래를 지원하려면 N(N-1)/2개의 풀이 필요하다. 5개만 해도 10개 풀, 20개면 190개 풀이다. 유동성이 분산되면 슬리피지가 높아지고 자본 효율성이 떨어진다.

### 2.2 기존 AMM의 한계

**Uniswap V3**: Concentrated liquidity를 도입했지만, 2개 자산 풀만 지원한다. 10개 스테이블코인을 거래하려면 45개의 개별 풀이 필요하다.

**Curve StableSwap**: N개 스테이블코인을 하나의 풀에서 거래할 수 있지만, 모든 LP가 동일한 유동성 프로파일을 갖는 uniform strategy만 지원한다. LP가 자신의 리스크 선호에 맞게 포지션을 커스터마이즈할 수 없다. 또한 하나의 코인이 depeg하면 풀 전체 유동성이 영향을 받는다.

**Orbital이 해결하는 것**: N개 자산을 하나의 풀에서 거래하면서(Curve의 장점), LP별로 다른 concentrated liquidity 포지션을 가질 수 있고(Uniswap V3의 장점), depeg 시 나머지 코인들이 격리되는 구조.

### 2.3 기관의 Stablecoin Settlement 마찰

기관(은행, 결제사, 거래소)은 고객 또는 거래 상대방의 stablecoin 선호가 다를 때 전환을 수행해야 한다.

현재 이 과정은: 수동 DEX 거래 → 슬리피지 발생 → compliance 기록 부재 → 다수 풀 경유. 규제 환경에서 운영하는 기관에게는 permissioned access, 거래 한도 설정, audit trail 같은 기능이 필수인데, 기존 AMM은 이를 제공하지 않는다.

---

## 3. Competitive Landscape

### 3.1 Orbital 구현체 현황

| 구현체 | 체인 | 특징 | 상태 |
|--------|------|------|------|
| Paradigm (원 논문) | N/A | 이론적 설계 | Research paper (2025.06) |
| agrawalx/orbital-pool | Arbitrum (Stylus/Rust) | 순수 AMM, 다중 LP per tick, Q96X48 fixed-point | 해커톤 프로젝트 |
| Rujira (THORChain) | CosmWasm AppLayer | Cross-chain stablecoin swap, TSS vault 기반 | 개발 중 (Q4 2025 soft launch 예정) |
| **본 프로젝트** | **Solana (Anchor/Rust)** | **Solana-native + institutional settlement** | **StableHacks MVP** |

### 3.2 차별점

**vs. agrawalx**: EVM 환경의 순수 AMM. Institutional layer 없음. Solana의 병렬 실행, 낮은 수수료, 빠른 finality를 활용하지 못함.

**vs. Rujira/THORChain**: Cross-chain에 초점. 단일 체인 내 기관 settlement이 아니라 chain 간 stablecoin 이동이 목적. Permissioned access 구조 없음.

**본 프로젝트**: Solana-native Orbital 구현 + permissioned settlement layer. 기관이 정책을 설정하고, 허용된 참가자만 settlement을 실행하며, 모든 거래가 on-chain audit trail로 남는 구조.

### 3.3 Solana 내 기존 Stable Swap

Jupiter, Orca (Whirlpool), Meteora는 모두 2-asset 풀 기반이다. N-asset pool에서 concentrated liquidity를 LP별로 커스터마이즈할 수 있는 프로토콜은 Solana에 존재하지 않는다.

---

## 4. Solution: Orbital Settlement Protocol

### 4.1 Architecture Overview

```
┌─────────────────────────────────────────────┐
│            Institutional Clients             │
│   (Banks, Payment Processors, Exchanges)     │
└──────────────────┬──────────────────────────┘
                   │
┌──────────────────▼──────────────────────────┐
│         Settlement API Layer                 │
│  ┌──────────┐ ┌───────────┐ ┌────────────┐  │
│  │ Policy   │ │ Allowlist │ │ Audit      │  │
│  │ Engine   │ │ Manager  │ │ Logger     │  │
│  └──────────┘ └───────────┘ └────────────┘  │
└──────────────────┬──────────────────────────┘
                   │
┌──────────────────▼──────────────────────────┐
│         Orbital AMM Core                     │
│  ┌──────────┐ ┌───────────┐ ┌────────────┐  │
│  │ Sphere   │ │ Tick      │ │ Torus      │  │
│  │ Invariant│ │ Manager  │ │ Trade Exec │  │
│  └──────────┘ └───────────┘ └────────────┘  │
└──────────────────┬──────────────────────────┘
                   │
┌──────────────────▼──────────────────────────┐
│              Solana Runtime                   │
│         (SPL Tokens on Devnet)               │
└─────────────────────────────────────────────┘
```

### 4.2 Layer 1: Orbital AMM Core

Paradigm 논문의 수학적 구조를 Solana Anchor program으로 구현한다.

**Sphere AMM Invariant**

기본 invariant:

```
||r⃗ - x⃗||² = Σᵢ(r - xᵢ)² = r²
```

reserve vector x가 중심 r⃗ = (r, r, ..., r), 반지름 r인 n차원 구의 표면 위에 존재해야 한다.

**Equal Price Point**

모든 reserve가 동일한 지점 q⃗ = (q, q, ..., q)이 $1 peg 상태:

```
q = r(1 - 1/√n)
```

**Token Price Derivation**

자산 j를 xᵢ 단위로 표현한 순간 가격:

```
δxᵢ/δxⱼ = (r - xⱼ)/(r - xᵢ)
```

reserve가 높은 자산은 가격이 낮고, reserve가 낮은 자산은 가격이 높다.

**Polar Reserve Decomposition**

임의의 reserve state x⃗를 equal-price 방향 v⃗ = (1/√n)(1,1,...,1)에 대해 분해:

```
x⃗ = αv⃗ + w⃗  (v⃗ ⊥ w⃗)
```

α를 고정하면 직교 부분공간에서 반지름 s = √(r² - (α - r√n)²)인 저차원 sphere AMM이 된다.

**Nested Tick Structure**

Orbital의 tick은 Uniswap V3와 다르다. $1 equal price point를 중심으로 한 n차원 spherical cap이며, 큰 tick이 작은 tick을 완전히 포함하는 nested 구조다.

각 tick은 평면 x⃗·v⃗ = k로 정의되며:
- k_min = r(√n - 1): 최소 tick (equal price point)
- k_max = r(n-1)/√n: 최대 tick

작은 tick은 $1 근처에만 유동성을 집중시켜 자본 효율성이 높다. 큰 tick은 depeg 시나리오도 커버하지만 자본 효율성은 낮다. LP는 자신의 리스크 선호에 따라 tick 크기를 선택한다.

**Tick Consolidation → Torus Trade Invariant**

Interior tick(가격이 $1 근처일 때 활성)들은 기하학적으로 유사하므로 하나의 구로 consolidate. Boundary tick(가격이 벗어났을 때 활성)들은 하나의 원으로 consolidate. 구를 원 주위로 회전시키면 torus(도넛) 형태의 단일 invariant가 된다.

이 torus invariant 덕분에 tick 수와 무관하게 constant-time에 거래를 계산할 수 있다. 큰 거래가 tick 경계를 넘을 때만 invariant를 업데이트한다.

**Depeg Isolation**

2D concentrated liquidity와 달리, 하나의 스테이블코인이 0으로 depeg해도 Orbital tick은 나머지 코인들을 공정 가격에 거래할 수 있다. LP의 손실이 해당 코인의 비중으로 제한된다.

**구현 세부사항**

- Fixed-point arithmetic: Q64.64 또는 Q96.48 (Solana compute unit 제한 고려하여 최적화)
- Newton's method: torus invariant 풀이용, fallback strategy 포함
- Account structure: Pool state, Tick state, LP position을 별도 account로 분리 (Solana account model 최적화)

### 4.3 Layer 2: Institutional Settlement Layer

Orbital AMM Core 위에 기관용 기능을 추가한다.

**Policy Engine**

기관 관리자가 설정 가능한 정책:
- 허용 스테이블코인 목록 (예: USDC, PYUSD만 허용)
- 단일 거래 한도 (예: 최대 $1M per tx)
- 일일 거래 한도
- 최소/최대 settlement 금액

**Allowlist Manager**

- 기관 wallet allowlist 관리
- Role-based access: Admin(정책 설정), Operator(settlement 실행), Viewer(조회만)
- 허용되지 않은 wallet의 settlement 요청 거부

**Audit Trail**

모든 settlement 거래를 on-chain event로 기록:
- 거래 시각
- 송신자/수신자 wallet
- 입력 토큰/금액, 출력 토큰/금액
- 적용된 정책 ID
- 실행 슬리피지

기관의 compliance 보고 및 감사에 활용 가능.

---

## 5. MVP Scope (StableHacks 10-Day Sprint)

### 5.1 지원 토큰 (Devnet)

- USDC (devnet SPL token)
- USDT (devnet SPL token)
- PYUSD (devnet SPL token)

3-asset pool로 Orbital의 3차원 sphere invariant를 구현한다.

### 5.2 Core Functions

**Orbital AMM Core**

1. `initialize_pool`: 3-asset Orbital pool 생성, sphere 파라미터(r) 설정
2. `add_liquidity`: LP가 tick 크기(k)를 지정하여 유동성 공급
3. `swap`: 임의의 stablecoin pair 교환 (예: USDC → PYUSD), torus invariant 기반 가격 계산
4. `remove_liquidity`: LP position 해제 및 자산 인출

**Settlement Layer**

5. `create_policy`: 기관 관리자가 settlement 정책 생성
6. `manage_allowlist`: wallet 추가/제거
7. `execute_settlement`: 정책 검증 → Orbital swap 실행 → audit log 기록
8. `query_audit`: settlement 기록 조회

### 5.3 구현하지 않는 것 (Post-MVP)

- 4개 이상 토큰 풀 (수학은 N차원으로 확장 가능하나 MVP에서는 3으로 제한)
- Cross-chain settlement
- Fee distribution mechanism 고도화
- Governance
- Mainnet 배포

### 5.4 기술 스택

| 구성 요소 | 기술 |
|-----------|------|
| Smart Program | Anchor (Rust) on Solana |
| Fixed-point Math | Custom Q64.64 library |
| Frontend | Next.js + @solana/web3.js |
| Wallet | Phantom (devnet) |
| 배포 환경 | Solana Devnet |

---

## 6. Demo Scenario (3-Minute Video)

StableHacks 제출 요구사항: "문제 → 이유 → 테스트넷 앱이 어떻게 해결하는지"

### Scene 1: Problem (30초)

화면에 텍스트:

> "5개 스테이블코인 → 10개 풀 필요. 100개 → 4,950개."
> "기관은 compliance 없이 swap할 수 없다."

### Scene 2: Solution 소개 (20초)

Orbital Settlement Protocol 소개:
- Paradigm Orbital AMM → Solana 최초 구현
- 하나의 풀에서 모든 stablecoin 교환
- 기관용 permissioned settlement

### Scene 3: 기관 정책 설정 (30초)

관리자 화면에서:
1. Settlement policy 생성 — 허용 토큰: USDC, USDT, PYUSD
2. 거래 한도: $500,000 per transaction
3. Allowlist에 operator wallet 추가

### Scene 4: Settlement 실행 (40초)

Operator 화면에서:
1. Wallet에 100,000 USDC 보유
2. Settlement 요청: USDC → PYUSD
3. 트랜잭션 실행
4. 결과: 100,000 USDC → 99,982 PYUSD (1.8bp slippage)

비교 데이터 표시: "기존 DEX routing: ~5-15bp slippage"

### Scene 5: Audit Trail (20초)

Settlement 기록 조회:
- 거래 상세, 정책 준수 여부, 실행 수치 확인

### Scene 6: Why Orbital Matters (20초)

> "3개 토큰 → 1개 풀. 1,000개 토큰 → 여전히 1개 풀."
> "LP는 자신의 리스크 선호에 맞게 유동성을 집중할 수 있다."
> "Orbital Settlement Protocol — institutional stablecoin infrastructure on Solana."

### Scene 7: Closing (20초)

기술 스택 요약, 향후 확장 계획, 팀 소개

---

## 7. 10-Day Execution Plan

| 일차 | 목표 | 산출물 |
|------|------|--------|
| Day 1-2 | Orbital math core 구현 | Sphere invariant, tick structure, Q64.64 math library (Rust) |
| Day 3 | Swap execution 구현 | Torus trade computation, Newton's method, 단위 테스트 통과 |
| Day 4 | Pool management 완성 | add/remove liquidity, Devnet 배포 |
| Day 5 | Settlement layer 구현 | Policy engine, allowlist, audit logger |
| Day 6 | Frontend 시작 | Swap UI, 지갑 연결, 기본 거래 흐름 |
| Day 7 | 관리자 UI 구현 | Policy 설정 화면, allowlist 관리, audit 조회 |
| Day 8 | End-to-end 통합 테스트 | 전체 시나리오 devnet에서 반복 실행 |
| Day 9 | 비디오 촬영 | 3분 데모 비디오 제작 |
| Day 10 | 제출물 정리 | 문서화, 코드 정리, DoraHacks 제출 |

**Risk Mitigation**: Day 3 시점에서 Orbital sphere invariant가 compute unit 제한 내에서 동작하지 않을 경우, simplified StableSwap invariant로 fallback. 아키텍처는 invariant를 교체 가능하도록 모듈화하여 설계한다.

---

## 8. Target Users

### Primary: 규제 금융기관

- Crypto banks (AMINA Bank 등): 고객 대리 stablecoin settlement
- Payment processors: cross-border 결제 시 stablecoin 전환
- Corporate treasury: 보유 stablecoin 리밸런싱

### Secondary: DeFi 프로토콜

- Lending protocols: 담보 stablecoin 간 전환
- Stablecoin issuers: 자사 토큰의 유동성 확보
- DEX aggregators: Orbital pool을 routing 경로로 활용

---

## 9. Business Model & LP Economics

### 9.1 수익 구조

프로토콜은 두 개의 수익원을 가진다.

**Swap Fee**: 모든 settlement 거래에서 수취하는 수수료. 기본 1-3bp. 이 중 대부분이 LP에게 분배되고, 프로토콜이 일부를 수취한다.

**Settlement Premium**: 기관이 permissioned clearing layer를 사용하는 데 대한 추가 수수료. 기관은 compliance, audit trail, policy enforcement 기능에 대해 프리미엄을 지불할 의사가 있다. 이건 일반 DeFi AMM에는 없는 수익원이다.

### 9.2 LP(Market Maker) 수익성

LP 역할은 Keyrock 같은 기관 market maker 또는 프로토콜 treasury가 담당한다. 일반 사용자가 LP를 할 필요는 없다.

**Orbital의 capital efficiency가 LP 수익률을 근본적으로 바꾼다.**

핵심 논리: Orbital은 같은 자본으로 훨씬 깊은 유동성을 제공한다. 유동성이 깊으면 슬리피지가 낮아서 더 많은 거래가 이 풀로 라우팅된다. volume이 높아지면 fee 수익이 올라간다. 즉 같은 LP 자본 대비 fee yield가 높아지는 구조다.

3-asset pool, 5% depeg tick 기준 비교:

**Curve 3pool 시나리오**
LP 자본 $1M 투입 시 effective liquidity $1M. 풀 전체 TVL이 $1B일 때 내 지분은 0.1%. 일일 volume $100M, fee 1bp 기준 풀 전체 일일 fee는 $100K. 내 일일 수익은 $100. 연환산 수익률 약 3.65%.

**Orbital pool 시나리오**
LP 자본 $1M 투입 시 effective liquidity $22M (22x capital efficiency). 같은 volume 기준 내 effective 지분이 22배 높다. 슬리피지가 낮아 추가 volume 유입도 기대 가능. 동일 조건에서 내 일일 수익은 약 $2,200. 연환산 수익률 약 80%.

실제로는 경쟁 LP가 진입하면서 수익률이 수렴하겠지만, 균형점에서도 Curve 대비 LP 자본 효율이 높다는 구조적 우위는 유지된다.

**더 타이트한 tick을 쓰면:**
1% depeg tick → 110x efficiency → 같은 자본으로 110배 효과. 대신 1% 넘게 depeg하면 LP position이 boundary에 고정된다. USDC/USDT 같은 major stablecoin이면 이 리스크가 매우 낮다.

### 9.3 LP 리스크

**Depeg 리스크**: tick 경계를 넘는 depeg이 발생하면 LP의 reserve가 boundary에 고정된다. 하지만 Orbital의 depeg 격리 덕분에, 하나의 코인이 depeg해도 나머지 코인들은 공정 가격에 계속 거래된다. Curve에서는 하나의 depeg이 전체 풀에 영향을 주는 것과 대조적이다.

**다자산 예치 요건**: LP는 풀의 모든 자산을 비율에 맞게 예치해야 한다. 3-asset pool이면 USDC, USDT, PYUSD 모두 필요하다. 다만 virtual reserves 덕분에 실제 예치 금액은 전체 effective liquidity의 일부(5% depeg tick 기준 약 4.6%)에 불과하다. 향후 Zap-in 기능을 통해 단일 자산으로 진입 후 내부적으로 분배하는 방식도 지원할 수 있다.

**Smart contract 리스크**: 모든 on-chain 프로토콜에 공통적으로 존재하는 리스크. 감사(audit)와 테스트로 완화.

### 9.4 Volume은 어디서 오는가

일반 DeFi AMM과 달리, 이 프로토콜은 **기관 settlement이 volume의 원천**이다.

**1단계 (Launch)**: AMINA Bank pilot을 통한 고객 stablecoin settlement volume. 기관 결제, treasury rebalancing에서 발생하는 예측 가능한 거래량.

**2단계 (Growth)**: DEX aggregator 연동 (Jupiter 등)으로 일반 DeFi volume 유입. Orbital의 낮은 슬리피지로 인해 라우팅 우선순위를 확보.

**3단계 (Scale)**: 추가 기관 온보딩, cross-chain settlement, tokenized deposit 지원으로 volume 확대.

MM 입장에서 핵심은 "이 풀에 기관 settlement volume이 들어올 것이라는 확신"이다. AMINA Bank이 파트너이고 Keyrock이 생태계 파트너인 구조에서, 이 확신이 뒷받침된다.

### 9.5 Keyrock 파트너십 시너지

Keyrock은 기관 market maker로서 유동성 공급이 본업이다. Orbital Settlement Protocol과의 시너지:

Keyrock이 LP로 참여하여 Orbital pool에 유동성을 공급한다. Orbital의 capital efficiency 덕분에 적은 자본으로 깊은 유동성을 제공할 수 있다. AMINA Bank이 settlement volume을 공급한다. Keyrock은 fee 수익을 얻고, AMINA Bank 고객은 낮은 슬리피지로 settlement을 수행한다.

이 구조는 StableHacks 심사에서 "파트너 간 시너지가 있는 실현 가능한 비즈니스"로 평가받을 수 있다.

---

## 10. StableHacks Alignment

### 9.1 해커톤 요구사항 충족

| 요구사항 | 충족 방식 |
|----------|----------|
| Solana 기반 | Anchor program, Devnet 배포 |
| 팀 제출 | 팀 구성 (솔로 불가) |
| MVP/Prototype 필수 (concept-only 탈락) | Working devnet demo |
| 3분 비디오 (문제→이유→해결) | Scene 1-7 시나리오 |
| Institutional-grade | Permissioned access, policy engine, audit trail |
| Regulatory-aligned | Allowlist, 거래 한도, compliance logging |
| Production-ready 지향 | 모듈화된 아키텍처, 테스트 커버리지 |

### 9.2 파트너 Fit

| 파트너 | 관련성 |
|--------|--------|
| AMINA Bank | Settlement protocol → bank pilot 가능성 |
| Solana Foundation | Solana 최초 Orbital 구현 → 생태계 기여 |
| Solstice Labs | Institutional-grade DeFi infra 구축 방향 일치 |
| Fireblocks | Settlement API → Fireblocks custody 연동 확장 가능 |
| Keyrock | Institutional market making → Orbital pool 유동성 공급 |
| Steakhouse Financial | Treasury management → settlement layer 활용 |

---

## 11. Success Metrics (MVP)

**기술 지표**
- 3-asset Orbital pool devnet 배포 및 안정 동작
- Swap slippage: 100K USDC 기준 < 3bp
- Settlement 트랜잭션 실행 시간: < 1초 (Solana finality 포함)

**데모 지표**
- End-to-end settlement 시나리오 성공적 시연
- Policy engine을 통한 거래 제한 동작 확인
- Audit trail 조회 기능 동작 확인

---

## 12. Future Extensions (Post-Hackathon)

### 12.1 N-Asset Scaling

MVP의 3-asset pool을 10, 100, 1000+ asset pool로 확장. Orbital의 constant-time trade computation 덕분에 자산 수 증가에 따른 연산 비용 증가가 없다.

### 12.2 Cross-Chain Settlement

Wormhole 또는 기타 bridge를 통해 타 체인의 stablecoin을 Solana Orbital pool에서 settlement.

### 12.3 Tokenized Deposit Integration

은행 발행 tokenized deposit, CBDC를 Orbital pool에 포함. AMINA Bank pilot에서 실제 규제 토큰으로 확장.

### 12.4 Settlement API

Fireblocks, Keyrock 같은 기관 인프라와 직접 연동 가능한 REST/gRPC API layer. 기관 시스템에서 프로그래밍 방식으로 settlement을 트리거.

### 12.5 Advanced Risk Management

LP별 depeg tolerance 설정 (agrawalx 구현에서 영감), issuer별 risk scoring, 자동 rebalancing.

---

## 13. References

1. White, D., Robinson, D., Moallemi, C. (2025). *Orbital*. Paradigm. https://www.paradigm.xyz/2025/06/orbital
2. agrawalx. *orbital-pool*. Arbitrum Stylus 구현. https://github.com/agrawalx/orbital-pool
3. Nine Realms / Rujira. *Orbital Pools on THORChain App Layer*. https://medium.com/thorchain/cross-chain-curve-orbital-pools-on-thorchain-app-layer
4. Egorov, M. (2019). *StableSwap — efficient mechanism for Stablecoin liquidity*. Curve Finance.
5. Adams, H. et al. (2021). *Uniswap v3 Core*. Uniswap.

---

*Orbital Settlement Protocol — Institutional stablecoin infrastructure, powered by Paradigm's Orbital AMM, built on Solana.*
