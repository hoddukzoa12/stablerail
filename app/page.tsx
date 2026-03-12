import { Card } from "./components/ui/card";

export default function Home() {
  return (
    <div className="flex min-h-[calc(100vh-4rem)] items-center justify-center">
      <Card variant="glass" className="w-full max-w-md p-8 text-center">
        <h2 className="text-2xl font-semibold text-text-primary">Swap</h2>
        <p className="mt-2 text-text-secondary">Coming in Phase 3</p>
      </Card>
    </div>
  );
}
