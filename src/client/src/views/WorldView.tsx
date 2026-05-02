import React, { useEffect, useRef } from "react";
import { useParams } from "react-router-dom";
import { load } from "../engine";

export default function WorldView() {
  const { id = "" } = useParams();
  const loaded = useRef(false);

  useEffect(() => {
    if (loaded.current) {
      return;
    }

    loaded.current = true;
    void load();
  }, []);

  return <div id="engine" data-world-id={id}></div>;
}