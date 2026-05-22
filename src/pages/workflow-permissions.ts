export type WorkflowActionMap<State extends string, Action extends string> = Record<
  State,
  readonly Action[]
>;

export function createWorkflowActionChecker<State extends string, Action extends string>(
  allowedActions: WorkflowActionMap<State, Action>,
  state: State
): (action: Action) => boolean {
  const allowed = new Set<Action>(allowedActions[state]);
  return (action: Action): boolean => allowed.has(action);
}
