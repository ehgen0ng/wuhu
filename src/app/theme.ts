import {
  ActionIcon,
  Badge,
  Button,
  Card,
  PasswordInput,
  Switch,
  TextInput,
  createTheme,
} from "@mantine/core";

export const theme = createTheme({
  fontFamily:
    'Inter, "Segoe UI", "Microsoft YaHei", system-ui, -apple-system, BlinkMacSystemFont, sans-serif',
  headings: {
    fontFamily:
      'Inter, "Segoe UI", "Microsoft YaHei", system-ui, -apple-system, BlinkMacSystemFont, sans-serif',
    fontWeight: "800",
  },
  primaryColor: "steam",
  primaryShade: { light: 5, dark: 4 },
  defaultRadius: "md",
  cursorType: "pointer",
  luminanceThreshold: 0.25,
  colors: {
    steam: [
      "#e7f8ff",
      "#cfeeff",
      "#9bd4ff",
      "#66c0f4",
      "#44aeea",
      "#2c96d4",
      "#1f77aa",
      "#1b5f88",
      "#194f70",
      "#12384f",
    ],
  },
  components: {
    ActionIcon: ActionIcon.extend({
      defaultProps: {
        radius: "md",
        size: 42,
        variant: "light",
      },
    }),
    Badge: Badge.extend({
      defaultProps: {
        radius: "sm",
        variant: "light",
      },
      styles: {
        root: {
          fontWeight: 800,
        },
      },
    }),
    Button: Button.extend({
      defaultProps: {
        radius: "md",
      },
      styles: {
        root: {
          minHeight: 42,
          fontWeight: 800,
          borderColor: "rgba(143, 188, 221, 0.16)",
        },
      },
    }),
    Card: Card.extend({
      defaultProps: {
        radius: "md",
        withBorder: true,
      },
    }),
    PasswordInput: PasswordInput.extend({
      defaultProps: {
        radius: "md",
        size: "md",
      },
      styles: {
        input: {
          backgroundColor: "rgba(8, 18, 27, 0.72)",
          borderColor: "rgba(143, 188, 221, 0.18)",
          color: "#f5fbff",
        },
      },
    }),
    Switch: Switch.extend({
      defaultProps: {
        color: "steam",
      },
      classNames: {
        input: "wuhu-switch-input",
        thumb: "wuhu-switch-thumb",
        track: "wuhu-switch-track",
      },
    }),
    TextInput: TextInput.extend({
      defaultProps: {
        radius: "md",
        size: "md",
      },
      styles: {
        input: {
          backgroundColor: "rgba(8, 18, 27, 0.72)",
          borderColor: "rgba(143, 188, 221, 0.18)",
          color: "#f5fbff",
        },
      },
    }),
  },
});
