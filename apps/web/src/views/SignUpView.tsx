import React, { useState } from "react";
import { basicSignUp } from "../api/auth";

export default function SignUpView() {
  const [username, setUsername] = useState("");
  const [password, setPassword] = useState("");
  const [passwordConfirmation, setPasswordConfirmation] = useState("");
  const [status, setStatus] = useState("");

  const onSubmit = async (event: React.FormEvent) => {
    event.preventDefault();

    if (password !== passwordConfirmation) {
      setStatus("passwords do not match");
      return;
    }

    const result = await basicSignUp(username, password);
    setStatus(result);
  };

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
      <div>
        <label htmlFor="password-confirmation">Password Confirmation:</label>
        <input
          id="password-confirmation"
          type="password"
          value={passwordConfirmation}
          onChange={(event) => setPasswordConfirmation(event.target.value)}
        />
      </div>
      <button type="submit" className="btn">
        Sign Up
      </button>
      {status ? <p>{status}</p> : null}
    </form>
  );
}
