type SwitchProps = {
  checked: boolean;
  disabled?: boolean;
  title?: string;
  ariaLabel: string;
  onChange: (checked: boolean) => void;
};

export function Switch({ checked, disabled = false, title, ariaLabel, onChange }: SwitchProps) {
  return (
    <button
      type="button"
      className="toggle-switch"
      title={title}
      role="switch"
      aria-checked={checked}
      aria-label={ariaLabel}
      data-state={checked ? "checked" : "unchecked"}
      disabled={disabled}
      onClick={() => onChange(!checked)}
    >
      <span className="toggle-switch__thumb" aria-hidden="true" />
    </button>
  );
}
