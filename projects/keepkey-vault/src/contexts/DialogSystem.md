# Dialog System Documentation

The KeepKey Desktop v5 application uses a centralized dialog management system to handle multiple dialogs, their priorities, and window focus management.

## Overview

The dialog system consists of:

1. **DialogContext**: Core context that manages dialog queue and state
2. **DialogProvider**: Provider component that wraps the app
3. **useDialog**: Hook to access dialog functionality
4. **useCommonDialogs**: Helper hook for common dialog patterns

## Features

- **Priority-based queueing**: Dialogs have priorities (low, normal, high, critical)
- **Single dialog display**: Only one dialog shown at a time
- **Persistent dialogs**: Some dialogs can't be closed by clicking outside
- **Window focus management**: Critical dialogs can request app focus
- **Dynamic loading**: Dialogs are lazy-loaded for performance

## Usage

### Basic Dialog

```tsx
import { useDialog } from '../contexts/DialogContext';

function MyComponent() {
  const { show, hide } = useDialog();
  
  const showMyDialog = () => {
    show({
      id: 'my-dialog',
      component: MyDialogComponent,
      props: { title: 'Hello' },
      priority: 'normal'
    });
  };
  
  return <button onClick={showMyDialog}>Show Dialog</button>;
}
```

### Using Common Dialogs

```tsx
import { useCommonDialogs } from '../hooks/useCommonDialogs';

function MyComponent() {
  const { showOnboarding, showSettings } = useCommonDialogs();
  
  // Show onboarding with completion callback
  showOnboarding({
    onComplete: () => console.log('Onboarding completed!')
  });
}
```

### Creating a Dialog Component

Dialog components receive an `onClose` prop automatically:

```tsx
interface MyDialogProps {
  onClose: () => void;
  title: string;
}

export function MyDialog({ onClose, title }: MyDialogProps) {
  return (
    <DialogRoot open={true} onOpenChange={({ open }) => !open && onClose()}>
      <DialogContent>
        <DialogHeader>
          <DialogTitle>{title}</DialogTitle>
          <DialogCloseTrigger />
        </DialogHeader>
        <DialogBody>
          {/* Dialog content */}
        </DialogBody>
      </DialogContent>
    </DialogRoot>
  );
}
```

## Dialog Priorities

- **low**: Background dialogs, can be interrupted
- **normal**: Standard dialogs
- **high**: Important dialogs (e.g., onboarding)
- **critical**: Must be addressed immediately

## Window Focus Management

For critical dialogs that need user attention:

```tsx
show({
  id: 'critical-dialog',
  component: CriticalDialog,
  priority: 'critical',
  onOpen: requestAppFocus,    // Brings window to front
  onClose: releaseAppFocus    // Releases focus control
});
```

## Best Practices

1. **Unique IDs**: Always use unique dialog IDs
2. **Cleanup**: Dialogs are automatically removed when closed
3. **Props**: Pass all necessary data through props
4. **Persistence**: Use `persistent: true` for dialogs that shouldn't be dismissed easily
5. **Lazy Loading**: Use React.lazy() for better performance

## Backend Integration

The dialog system can be triggered by backend events:

```tsx
// Listen for backend events
useEffect(() => {
  const unlisten = listen('show-dialog', (event) => {
    const { dialogType, props } = event.payload;
    
    switch (dialogType) {
      case 'error':
        showError(props.title, props.message);
        break;
      case 'update':
        showUpdateDialog(props);
        break;
    }
  });
  
  return () => unlisten();
}, []);
``` 