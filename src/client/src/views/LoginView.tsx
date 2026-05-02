import React, { useState } from "react";
import { basicLogin } from "../api/auth";

export default function LoginView() {
  const [username, setUsername] = useState("");
  const [password, setPassword] = useState("");
  const [status, setStatus] = useState("");

  const onSubmit = async (event: React.FormEvent) => {
    event.preventDefault();
    const result = await basicLogin(username, password);
    setStatus(result);
  };

  return (
    <form onSubmit={onSubmit} className="card">
      <div>
        <label htmlFor="username">Username:</label>
        <input id="username" value={username} onChange={(event) => setUsername(event.target.value)} />
      </div>
      <div>
        <label htmlFor="password">Password:</label>
        <input
          id="password"
          type="password"
          value={password}
          onChange={(event) => setPassword(event.target.value)}
        />
      </div>
      <button type="submit" className="btn btn-success">
        Login
      </button>
      {status ? <p>{status}</p> : null}
    </form>
  );
}