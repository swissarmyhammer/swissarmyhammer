# TypeScript / JavaScript Testing Patterns

**Load this reference when**: working in a TypeScript or JavaScript project and applying TDD. Covers test structure, tooling, and TS/JS-specific patterns.

## Test Runner

Use **Vitest** with **Vite** for new projects or projects that haven't already chosen a test framework. It's fast (native ESM, Vite's transform pipeline), compatible with Jest's API, and has built-in TypeScript support with no extra config.

If the project already uses Jest, stay with Jest. Don't migrate mid-feature.

```bash
# Install
npm install -D vitest

# Run all tests
npx vitest run

# Watch mode (re-runs on file change)
npx vitest

# Run specific test file
npx vitest run src/service.test.ts

# Run tests matching name
npx vitest run -t "should create user"

# Coverage
npx vitest run --coverage
```

### Minimal Config

```typescript
// vitest.config.ts
import { defineConfig } from 'vitest/config';

export default defineConfig({
  test: {
    globals: true,
    environment: 'node', // or 'jsdom' for browser/UI code
    coverage: {
      provider: 'v8',
      thresholds: { branches: 80, functions: 80, lines: 80, statements: 80 },
    },
  },
});
```

## Test Structure

```typescript
import { describe, it, expect, beforeEach } from 'vitest';

describe('UserService', () => {
  let service: UserService;

  beforeEach(() => {
    service = new UserService();
  });

  describe('create', () => {
    it('should assign an id when creating a user', () => {
      // Arrange
      const input = { name: 'Alice', email: 'alice@example.com' };

      // Act
      const user = service.create(input);

      // Assert
      expect(user.id).toBeDefined();
    });

    it('should throw when email already exists', () => {
      service.create({ name: 'Alice', email: 'alice@example.com' });

      expect(() =>
        service.create({ name: 'Bob', email: 'alice@example.com' })
      ).toThrow('already exists');
    });
  });
});
```

### Test Naming

Use descriptive `it('should [behavior] when [condition]')`:

```typescript
it('should return empty array when no users exist')
it('should reject when token is expired')
it('should trim whitespace from email before saving')
```

## Mocking

### Dependency Injection (Preferred)

Design code to accept dependencies rather than creating them internally:

```typescript
// Production
interface PaymentGateway {
  charge(amount: number): Promise<Receipt>;
}

class OrderService {
  constructor(private payments: PaymentGateway) {}

  async checkout(order: Order): Promise<Receipt> {
    return this.payments.charge(order.total);
  }
}

// Test
it('should charge the order total', async () => {
  const mockPayments: PaymentGateway = {
    charge: vi.fn().mockResolvedValue({ id: 'receipt-1' }),
  };

  const service = new OrderService(mockPayments);
  const receipt = await service.checkout({ total: 50 });

  expect(receipt.id).toBe('receipt-1');
  expect(mockPayments.charge).toHaveBeenCalledWith(50);
});
```

### Module Mocking (System Boundaries Only)

```typescript
import { vi } from 'vitest';

// Mock an external HTTP client
vi.mock('./http-client', () => ({
  get: vi.fn(),
  post: vi.fn(),
}));
```

### Spies

```typescript
const spy = vi.spyOn(logger, 'warn');
service.processInvalid(input);
expect(spy).toHaveBeenCalledWith(expect.stringContaining('invalid'));
spy.mockRestore();
```

### Fake Timers

```typescript
it('should expire after timeout', () => {
  vi.useFakeTimers();

  const session = createSession({ ttl: 3600_000 });
  expect(session.isExpired()).toBe(false);

  vi.advanceTimersByTime(3600_001);
  expect(session.isExpired()).toBe(true);

  vi.useRealTimers();
});
```

## Async Testing

```typescript
it('should fetch user by id', async () => {
  const user = await service.getUser('123');
  expect(user.name).toBe('Alice');
});

it('should reject when user not found', async () => {
  await expect(service.getUser('999')).rejects.toThrow('not found');
});
```

## UI Testing

Separate logic from presentation. TDD the logic layer. Snapshot or render-test the presentation layer.

### Testing Hooks (React)

```typescript
import { renderHook, act } from '@testing-library/react';

it('should increment counter', () => {
  const { result } = renderHook(() => useCounter(0));

  act(() => result.current.increment());

  expect(result.current.count).toBe(1);
});
```

### Testing Components (Behavior, Not Markup)

```typescript
import { render, screen, fireEvent } from '@testing-library/react';

it('should call onSubmit with form data', () => {
  const onSubmit = vi.fn();
  render(<UserForm onSubmit={onSubmit} />);

  fireEvent.change(screen.getByLabelText('Name'), { target: { value: 'Alice' } });
  fireEvent.click(screen.getByRole('button', { name: 'Submit' }));

  expect(onSubmit).toHaveBeenCalledWith({ name: 'Alice' });
});
```

Query priority: `getByRole` > `getByLabelText` > `getByPlaceholderText` > `getByTestId`. Prefer semantic queries that reflect how users interact with the UI.

### Snapshot Testing (Presentation Layer)

```typescript
it('should render user card correctly', () => {
  const { container } = render(<UserCard user={testUser} />);
  expect(container.firstChild).toMatchSnapshot();
});
```

Update snapshots intentionally with `npx vitest run --update`.

## Property-Based Testing

Use `fast-check` for property-based testing:

```typescript
import fc from 'fast-check';

it('should roundtrip JSON serialize/deserialize', () => {
  fc.assert(
    fc.property(fc.jsonValue(), (value) => {
      expect(JSON.parse(JSON.stringify(value))).toEqual(value);
    })
  );
});
```

## Test Fixtures / Factories

```typescript
function buildUser(overrides?: Partial<User>): User {
  return {
    id: crypto.randomUUID(),
    name: 'Test User',
    email: 'test@example.com',
    createdAt: new Date(),
    ...overrides,
  };
}

// Usage
const user = buildUser({ name: 'Alice' });
```

## Integration Tests (API)

```typescript
import request from 'supertest';

describe('POST /api/users', () => {
  it('should create user and return 201', async () => {
    const res = await request(app)
      .post('/api/users')
      .send({ name: 'Alice', email: 'alice@example.com' })
      .expect(201);

    expect(res.body).toHaveProperty('id');
    expect(res.body.name).toBe('Alice');
  });

  it('should return 400 for invalid email', async () => {
    await request(app)
      .post('/api/users')
      .send({ name: 'Alice', email: 'not-an-email' })
      .expect(400);
  });
});
```

## Coverage

```bash
npx vitest run --coverage                  # Generate report
npx vitest run --coverage --reporter=html  # HTML report
```

In `vitest.config.ts`, set thresholds to enforce minimums. CI should fail if coverage drops below the threshold.
