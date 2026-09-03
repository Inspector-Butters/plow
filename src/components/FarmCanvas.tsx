export function FarmCanvas() {
  return (
    <div className="farm-canvas" aria-hidden="true">
      <img className="farm-canvas__surface" src="/assets/plow-farm-v2.png" alt="" />
      <div className="farm-canvas__sunbeam" />
      <div className="farm-canvas__pollen farm-canvas__pollen--one" />
      <div className="farm-canvas__pollen farm-canvas__pollen--two" />
      <div className="farm-canvas__pollen farm-canvas__pollen--three" />
    </div>
  );
}
