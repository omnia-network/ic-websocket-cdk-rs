/**
 * Sleeps for the specified number of milliseconds.
 * @param ms Number of milliseconds to sleep
 */
export const sleep = (ms: number) => new Promise((resolve) => setTimeout(resolve, ms));
