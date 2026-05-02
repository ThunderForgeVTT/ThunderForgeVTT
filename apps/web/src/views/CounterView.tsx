import React, { useState } from "react";

export default function CounterView() {
  const [value, setValue] = useState(0);

  return (
    <div>
      <button onClick={() => setValue((current) => current + 1)}>+1</button>
      <p>{value}</p>
    </div>
  );
}