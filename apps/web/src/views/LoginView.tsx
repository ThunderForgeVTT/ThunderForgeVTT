import React, { useState } from "react";
import { basicLogin } from "../api/auth";
import { useSetupStatus } from "../hooks/useSetupStatus";

export default function LoginView() {
  const { setupStatus, isLoading } = useSetupStatus();
  const [username, setUsername] = useState("");
  const [password, setPassword] = useState("");
  const [status, setStatus] = useState("");

  const onSubmit = async (event: React.FormEvent) => {
    event.preventDefault();
    const result = await basicLogin(username, password);
    setStatus(result);
  };

  if (isLoading) {
    return <div>Loading...</div>;
  }

  if (setupStatus?.setup_required) {
    return (
      <div className="card">
        <h2>Instance Not Configured</h2>
        <p>
          This ThunderForgeVTT instance has not been configured. Please check the
          server console for the administrator bootstrap code and navigate to{" "}
          <a href="/setup">/setup</a> to complete this process.
        </p>
      </div>
    );
  }

  return (
    <form onSubmit={onSubmit} className="card">
      <div>
        <label htmlFor="username">Username:</label>
        <input
          id="username"
          value={username}
          onChange={(event) => setUsername(event.target.value)}
        />
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
