# Dashboard Refactoring Plan

## 요약
3가지 변경: Add Liquidity → 모달, Positions 카드에 버튼그룹, 우측에 Tokens 잔액 카드

## 현재 레이아웃
```
[Pool Header]
[Donut Chart (left)]        [Swap/Add buttons + Stats (right)]
[Add Liquidity Form (left)] [Your Positions (right)]           ← wallet gated
```

## 변경 후 레이아웃
```
[Pool Header]
[Donut Chart (left)]                    [Swap/Add buttons + Stats (right)]
[Positions + 버튼그룹 (left, 넓게)]     [Tokens 잔액 카드 (right, 340px)]   ← wallet gated
```

## 구현 단계

### Step 1: Modal UI 컴포넌트 생성
- **파일**: `app/components/ui/modal.tsx`
- Portal 기반 (createPortal → document.body)
- Backdrop overlay (click-to-close) + ESC 키 닫기
- glass-panel 스타일 + 닫기(X) 버튼
- Props: `open`, `onClose`, `title`, `children`

### Step 2: AddLiquidityForm을 Modal 안으로
- `add-liquidity-form.tsx`에서 Card wrapper 제거 → 순수 form content만 남김
- 모달 open/close 상태는 dashboard page에서 관리
- 상단 "Add liquidity" 버튼 → `onClick={openModal}` 로 변경 (기존 `<a href="#add-liquidity">` 제거)

### Step 3: Positions 카드에 버튼그룹 추가
- `user-positions.tsx` 상단에 버튼그룹 추가:
  - "+ Add Liquidity" (gradient, 모달 열기) — `onAddLiquidity` callback prop
  - "Swap" (secondary, Link to `/`)
- 기존 각 포지션의 "Remove" 버튼 유지

### Step 4: TokensCard 컴포넌트 생성
- **파일**: `app/components/dashboard/tokens-card.tsx`
- Props: `balances: Record<string, bigint>`
- USDC/USDT/PYUSD 각각 color dot + symbol + 잔액 표시
- 잔액 0이면 "—" 표시

### Step 5: Dashboard page 레이아웃 업데이트
- 하단 2-column: `md:grid-cols-[1fr_340px]` (상단과 동일 비율)
- Left: `<UserPositions>` (넓어짐, `onAddLiquidity` prop 전달)
- Right: `<TokensCard>`
- Modal 상태: `const [modalOpen, setModalOpen] = useState(false)`
- `<Modal>` 안에 `<AddLiquidityForm>` 렌더링
- 미연결 상태: 기존 "Connect wallet" 메시지 유지

## 파일 변경 목록
| 파일 | 작업 |
|------|------|
| `app/components/ui/modal.tsx` | 신규 생성 |
| `app/components/dashboard/tokens-card.tsx` | 신규 생성 |
| `app/components/dashboard/add-liquidity-form.tsx` | Card wrapper → div로 변경 |
| `app/components/dashboard/user-positions.tsx` | 버튼그룹 추가, onAddLiquidity prop |
| `app/dashboard/page.tsx` | Modal 상태, 레이아웃 변경 |
