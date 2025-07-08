// TypeScript code with type and pattern issues

interface User {
    name: string;
    email: string;
}

// Bad: using any type
function processUser(user: any): any {
    console.log("Processing user:", user);
    return user;
}

// Bad: any array
let userData: any[] = [];

// Bad: any in arrow function
const transform = (data: any) => data.toString();

// Good examples (should not be detected)
function processUserCorrect(user: User): User {
    console.log("Processing user:", user);
    return user;
}

let userDataCorrect: User[] = [];

// Defensive method calls that could use optional chaining
function handleCallback(callback?: Function, data?: any) {
    // Should be replaced with optional chaining
    callback && callback(data);

    // Another pattern
    data.method && data.method();

    // This is already good
    callback?.(data);
}

// React-style hook usage (for hook dependency rule)
function useCustomHook(dependencies: string[]) {
    const [state, setState] = useState(null);

    // This might have dependency issues
    useEffect(() => {
        const value = dependencies[0];
        setState(value);
    }, []); // Missing dependencies array

    return state;
}
