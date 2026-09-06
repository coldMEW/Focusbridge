import { useId, useState, type InputHTMLAttributes } from "react";

type PasswordInputProps = Omit<InputHTMLAttributes<HTMLInputElement>, "type"> & {
  label: string;
  wrapperClassName?: string;
};

export default function PasswordInput({
  label,
  id,
  className = "",
  wrapperClassName = "",
  ...props
}: PasswordInputProps) {
  const generatedId = useId();
  const inputId = id ?? generatedId;
  const [visible, setVisible] = useState(false);
  const accessibleLabel = props["aria-label"]?.trim() || label.trim() || "Password";
  const toggleLabel = `${visible ? "Hide" : "Show"} ${accessibleLabel}`;

  return (
    <div className={`relative ${wrapperClassName}`}>
      <input
        {...props}
        aria-label={accessibleLabel}
        id={inputId}
        type={visible ? "text" : "password"}
        className={`${className} !pr-12`}
      />
      <button
        type="button"
        aria-label={toggleLabel}
        aria-controls={inputId}
        title={toggleLabel}
        disabled={props.disabled}
        onClick={() => setVisible((current) => !current)}
        className="absolute right-2 top-1/2 grid h-9 w-9 -translate-y-1/2 place-items-center rounded-lg text-text-secondary transition hover:bg-bg-secondary hover:text-text-primary focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-accent-study focus-visible:ring-offset-2 focus-visible:ring-offset-bg-primary disabled:cursor-not-allowed disabled:opacity-50"
      >
        <svg aria-hidden="true" focusable="false" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.8" strokeLinecap="round" strokeLinejoin="round" className="h-5 w-5">
          <path d="M2 12s3.5-7 10-7 10 7 10 7-3.5 7-10 7S2 12 2 12Z" />
          <circle cx="12" cy="12" r="3" />
          {visible && <path d="m3 3 18 18" />}
        </svg>
      </button>
    </div>
  );
}
