export { cn } from "cnfast";

/**
 * returns error message or string representation of error
 */
export function parseError(error: unknown) {
  if (error instanceof Error) {
    return error.message;
  }
  return String(error);
}

/**
 * capitalize first character of given string
 *
 * @param data - string to modify
 */
export const upperFirstLetter = (data: string = "") => {
  return data.slice(0, 1).toUpperCase() + data.slice(1, data.length);
};
