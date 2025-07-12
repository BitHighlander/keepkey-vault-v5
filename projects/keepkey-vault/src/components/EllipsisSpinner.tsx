// EllipsisDots.tsx
import React, { useEffect, useState } from "react";
import { Text } from "@chakra-ui/react";

interface EllipsisDotsProps {
    interval?: number;      // ms between frames (default 300 ms)
    color?: string;         // inherits if omitted
}

/**
 * Animated ". .. ..." spinner that doesn’t shift layout.
 */
export const EllipsisDots: React.FC<EllipsisDotsProps> = ({
                                                              interval = 300,
                                                              color = "currentColor",
                                                          }) => {
    const [step, setStep] = useState(1);          // 1, 2, 3 dots

    useEffect(() => {
        const id = setInterval(
            () => setStep((s) => (s === 3 ? 1 : s + 1)),
            interval
        );
        return () => clearInterval(id);
    }, [interval]);

    // Build the dot string for the current frame
    const dots = ".".repeat(step);

    return (
        <Text
            as="span"
            display="inline-block"
            w="3ch"              /* reserve 3 character‐cells */
            textAlign="left"     /* left-align so dots don’t “slide” */
            color={color}
        >
            {dots}
        </Text>
    );
};
