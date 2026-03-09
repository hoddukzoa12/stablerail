# Orbital Settlement Protocol — Design System

**Version 1.0 — March 2026**

---

## 1. Design Principles

| 원칙 | 설명 |
|------|------|
| **Institutional Trust** | 기관 사용자가 신뢰할 수 있는 정제된 다크 인터페이스 |
| **Data Clarity** | 숫자, 가격, 슬리피지 등 금융 데이터를 명확하게 표현 |
| **Minimal Friction** | Settlement 워크플로우의 단계를 최소화하고 직관적으로 |
| **Solana Native** | Solana/Jupiter 생태계와 시각적으로 조화 |

---

## 2. Color System

### 2.1 Brand Colors

| Token | Hex | 용도 |
|-------|-----|------|
| `brand-primary` | `#9945FF` | 주요 CTA, 액센트, 활성 상태 |
| `brand-primary-hover` | `#AD6AFF` | Primary hover 상태 |
| `brand-primary-pressed` | `#7C2FE6` | Primary pressed 상태 |
| `brand-secondary` | `#14F195` | 성공, 긍정 지표, Solana 그린 |
| `brand-gradient` | `linear-gradient(135deg, #9945FF, #14F195)` | 로고, 강조 배지, 히어로 |

### 2.2 Surface Colors (Dark Theme)

| Token | Hex | 용도 |
|-------|-----|------|
| `surface-base` | `#0B0B0F` | 최하단 배경 |
| `surface-1` | `#111116` | 카드, 패널 배경 |
| `surface-2` | `#1A1A22` | 입력 필드, 드롭다운 배경 |
| `surface-3` | `#22222E` | Hover 상태, 선택된 행 |
| `surface-overlay` | `rgba(0, 0, 0, 0.60)` | 모달 오버레이 |

### 2.3 Border Colors

| Token | Hex | 용도 |
|-------|-----|------|
| `border-default` | `#1E1E2A` | 기본 구분선 |
| `border-subtle` | `#16161F` | 약한 구분선 |
| `border-focus` | `#9945FF` | Focus ring |
| `border-gradient` | `linear-gradient(135deg, rgba(153,69,255,0.3), rgba(20,241,149,0.3))` | 강조 카드 테두리 |

### 2.4 Text Colors

| Token | Hex | 용도 |
|-------|-----|------|
| `text-primary` | `#E4E4E7` | 본문, 제목 |
| `text-secondary` | `#A1A1AA` | 보조 텍스트, 레이블 |
| `text-tertiary` | `#71717A` | 비활성, 플레이스홀더 |
| `text-inverse` | `#0B0B0F` | 밝은 배경 위 텍스트 |
| `text-brand` | `#9945FF` | 브랜드 강조 텍스트 |

### 2.5 Semantic Colors

| Token | Hex | 용도 |
|-------|-----|------|
| `semantic-success` | `#14F195` | 성공, 양수 변화 |
| `semantic-success-bg` | `rgba(20, 241, 149, 0.10)` | 성공 배경 |
| `semantic-error` | `#FF4D4D` | 에러, 음수 변화, 거부 |
| `semantic-error-bg` | `rgba(255, 77, 77, 0.10)` | 에러 배경 |
| `semantic-warning` | `#FFB020` | 경고, 주의 |
| `semantic-warning-bg` | `rgba(255, 176, 32, 0.10)` | 경고 배경 |
| `semantic-info` | `#38BDF8` | 정보, 알림 |
| `semantic-info-bg` | `rgba(56, 189, 248, 0.10)` | 정보 배경 |

### 2.6 Token Indicator Colors

스테이블코인 구분용 색상:

| Token | Color | Hex |
|-------|-------|-----|
| USDC | Blue | `#2775CA` |
| USDT | Green | `#26A17B` |
| PYUSD | Blue Dark | `#0033A0` |

---

## 3. Typography

### 3.1 Font Family

```css
--font-sans: 'Inter', -apple-system, BlinkMacSystemFont, 'Segoe UI', sans-serif;
--font-mono: 'JetBrains Mono', 'Fira Code', 'Consolas', monospace;
```

- **Inter**: UI 전반, 본문, 레이블
- **JetBrains Mono**: 숫자, 주소, 해시, 코드

### 3.2 Type Scale

| Token | Size | Line Height | Weight | 용도 |
|-------|------|-------------|--------|------|
| `display` | 36px | 40px | 600 | 히어로 헤드라인 |
| `heading-1` | 28px | 36px | 600 | 페이지 제목 |
| `heading-2` | 22px | 28px | 600 | 섹션 제목 |
| `heading-3` | 18px | 24px | 600 | 카드 제목 |
| `body-lg` | 16px | 24px | 400 | 큰 본문 |
| `body` | 14px | 20px | 400 | 기본 본문 |
| `body-sm` | 13px | 18px | 400 | 보조 텍스트 |
| `caption` | 12px | 16px | 500 | 레이블, 배지 |
| `overline` | 11px | 16px | 600 | 카테고리, 상태 |
| `number-lg` | 28px | 36px | 600 | 대형 숫자 (TVL, 잔액) |
| `number` | 16px | 24px | 500 | 일반 숫자 |
| `number-sm` | 13px | 18px | 500 | 테이블 내 숫자 |

### 3.3 숫자 표기 규칙

| 항목 | 형식 | 예시 |
|------|------|------|
| 금액 (USD) | `$` + 콤마 구분 | `$1,234,567.89` |
| 토큰 수량 | 콤마 구분 + 심볼 | `100,000.00 USDC` |
| 슬리피지 | 소수점 1자리 + bp | `1.8 bp` |
| 퍼센트 | 소수점 2자리 + % | `0.02%` |
| 주소 | 앞 4 + ... + 뒤 4 | `7nYB...x3Kp` |

---

## 4. Spacing System

**Base unit**: 4px

| Token | Value | 용도 |
|-------|-------|------|
| `space-1` | 4px | 아이콘-텍스트 간격 |
| `space-2` | 8px | 인라인 요소 간격 |
| `space-3` | 12px | 컴팩트 패딩 |
| `space-4` | 16px | 카드 내부 패딩 |
| `space-5` | 20px | 섹션 간 간격 |
| `space-6` | 24px | 카드 패딩 |
| `space-8` | 32px | 섹션 구분 |
| `space-10` | 40px | 페이지 섹션 간격 |
| `space-12` | 48px | 대형 섹션 간격 |
| `space-16` | 64px | 페이지 상하 여백 |

---

## 5. Border Radius

| Token | Value | 용도 |
|-------|-------|------|
| `radius-sm` | 6px | 배지, 태그, 칩 |
| `radius-md` | 8px | 버튼, 입력 필드 |
| `radius-lg` | 12px | 카드, 패널 |
| `radius-xl` | 16px | 모달, 대형 카드 |
| `radius-2xl` | 20px | Swap 카드 (메인 UI) |
| `radius-full` | 9999px | 아바타, 토글, 상태 인디케이터 |

---

## 6. Shadows & Effects

### 6.1 Shadows

| Token | Value | 용도 |
|-------|-------|------|
| `shadow-sm` | `0 1px 2px rgba(0,0,0,0.3)` | 버튼, 입력 |
| `shadow-md` | `0 4px 12px rgba(0,0,0,0.4)` | 카드, 드롭다운 |
| `shadow-lg` | `0 8px 24px rgba(0,0,0,0.5)` | 모달, 팝오버 |
| `shadow-glow` | `0 0 20px rgba(153,69,255,0.15)` | 브랜드 강조 글로우 |

### 6.2 Glassmorphism (Jupiter 스타일)

```css
.glass-panel {
  background: rgba(17, 17, 22, 0.80);
  backdrop-filter: blur(12px);
  border: 1px solid rgba(255, 255, 255, 0.06);
}
```

### 6.3 Gradient Border (강조 카드)

```css
.gradient-border {
  position: relative;
  background: var(--surface-1);
  border-radius: var(--radius-lg);
}
.gradient-border::before {
  content: '';
  position: absolute;
  inset: 0;
  padding: 1px;
  border-radius: inherit;
  background: linear-gradient(135deg, rgba(153,69,255,0.4), rgba(20,241,149,0.4));
  -webkit-mask: linear-gradient(#fff 0 0) content-box, linear-gradient(#fff 0 0);
  -webkit-mask-composite: xor;
  mask-composite: exclude;
}
```

---

## 7. Component Tokens

### 7.1 Button Variants

| Variant | Background | Text | Border |
|---------|-----------|------|--------|
| Primary | `brand-primary` | `text-inverse` | none |
| Secondary | `surface-2` | `text-primary` | `border-default` |
| Ghost | transparent | `text-secondary` | none |
| Danger | `semantic-error` | `#FFFFFF` | none |
| Gradient | `brand-gradient` | `#FFFFFF` | none |

**States**: hover (+8% lightness), pressed (-4% lightness), disabled (40% opacity)

**Sizes**:

| Size | Height | Padding X | Font |
|------|--------|-----------|------|
| sm | 32px | 12px | `caption` |
| md | 40px | 16px | `body` |
| lg | 48px | 24px | `body-lg` |

### 7.2 Input Fields

```
Background: surface-2
Border: border-default (1px)
Border Focus: brand-primary (1.5px)
Padding: 12px 16px
Height: 48px (default), 56px (large, swap input)
Font: body (label), number (value)
Placeholder: text-tertiary
```

### 7.3 Card

```
Background: surface-1
Border: border-default (1px)
Border Radius: radius-lg (12px)
Padding: space-6 (24px)
Shadow: shadow-md (hover)
```

### 7.4 Swap Card (메인 UI)

```
Background: surface-1
Border Radius: radius-2xl (20px)
Padding: space-6 (24px)
Inner Token Selector: surface-2, radius-md
Amount Input: font-mono, number-lg (28px)
Swap Button (가운데): 40x40, radius-full, surface-2, hover:brand-primary
```

### 7.5 Table (Audit Trail)

```
Header: text-tertiary, overline (11px), uppercase
Row: surface-1, border-bottom border-subtle
Row Hover: surface-3
Cell Font: body-sm (text), number-sm (숫자)
Cell Padding: 12px 16px
```

### 7.6 Badge / Status

| Status | Background | Text |
|--------|-----------|------|
| Executed | `semantic-success-bg` | `semantic-success` |
| Pending | `semantic-warning-bg` | `semantic-warning` |
| Rejected | `semantic-error-bg` | `semantic-error` |
| Active | `brand-primary` + 10% bg | `brand-primary` |

---

## 8. Layout

### 8.1 Page Structure

```
┌──────────────────────────────────────────────┐
│  Top Nav (h: 64px)                           │
│  Logo | Navigation | Wallet Connect          │
├──────────────────────────────────────────────┤
│                                              │
│  Main Content (max-width: 1200px, centered)  │
│                                              │
│  ┌─────────┐  ┌──────────────────────────┐   │
│  │ Sidebar │  │ Content Area             │   │
│  │ (240px) │  │                          │   │
│  │         │  │                          │   │
│  └─────────┘  └──────────────────────────┘   │
│                                              │
└──────────────────────────────────────────────┘
```

### 8.2 Responsive Breakpoints

| Token | Value | 타겟 |
|-------|-------|------|
| `mobile` | < 640px | 모바일 |
| `tablet` | 640–1024px | 태블릿 |
| `desktop` | > 1024px | 데스크탑 |
| `wide` | > 1400px | 와이드 모니터 |

### 8.3 Navigation 구조

```
Swap         — /              (메인 Swap UI)
Dashboard    — /dashboard     (Pool 통계)
Settlement   — /settlement    (기관 Operator)
Admin        — /admin         (Policy 관리)
```

---

## 9. Motion & Animation

| Token | Value | 용도 |
|-------|-------|------|
| `duration-fast` | 100ms | Hover, 색상 전환 |
| `duration-normal` | 200ms | 버튼, 토글, 드롭다운 |
| `duration-slow` | 300ms | 모달, 패널, 페이지 전환 |
| `easing-default` | `cubic-bezier(0.4, 0, 0.2, 1)` | 대부분의 전환 |
| `easing-spring` | `cubic-bezier(0.34, 1.56, 0.64, 1)` | 바운스 효과 (Swap 버튼) |

---

## 10. Iconography

- **라이브러리**: Lucide Icons (React)
- **사이즈**: 16px (inline), 20px (default), 24px (navigation)
- **스트로크**: 1.5px
- **색상**: `text-secondary` 기본, hover 시 `text-primary`

---

## 11. Tailwind CSS Config Reference

```js
// tailwind.config.ts 에 반영할 핵심 토큰
colors: {
  brand: {
    primary: '#9945FF',
    'primary-hover': '#AD6AFF',
    'primary-pressed': '#7C2FE6',
    secondary: '#14F195',
  },
  surface: {
    base: '#0B0B0F',
    1: '#111116',
    2: '#1A1A22',
    3: '#22222E',
  },
  border: {
    default: '#1E1E2A',
    subtle: '#16161F',
  },
  text: {
    primary: '#E4E4E7',
    secondary: '#A1A1AA',
    tertiary: '#71717A',
  },
  success: '#14F195',
  error: '#FF4D4D',
  warning: '#FFB020',
  info: '#38BDF8',
}
```

---

*Orbital Settlement Protocol Design System — Dark, Purple/Violet, Jupiter-inspired DeFi aesthetic.*
