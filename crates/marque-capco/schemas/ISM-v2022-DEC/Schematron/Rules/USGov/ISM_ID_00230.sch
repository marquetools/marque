<?xml version="1.0" encoding="UTF-8"?>
<?ICEA pattern?>
<?schematron-phases phaseids="ROLLDOWN VALUECHECK"?>
<!-- Notices - Distribution Notice: 
           This document has been approved for Public Release and is available for use without restriction.
       -->
<sch:pattern xmlns:sch="http://purl.oclc.org/dsdl/schematron" id="ISM-ID-00230">
    <sch:p xmlns:ism="urn:us:gov:ic:ism" ism:classification="U" ism:ownerProducer="USA" class="ruleText">
        [ISM-ID-00230][Error] If ISM_USGOV_RESOURCE and attribute @ism:atomicEnergyMarkings of ISM_RESOURCE_ELEMENT contains 
        [FRD-SG-##] then at least one element meeting ISM_CONTRIBUTES in the document must have a 
        @ism:atomicEnergyMarking attribute containing the same [FRD-SG-##].
        
        Human Readable: USA documents marked FRD-SG-## at the resource level must have FRD-SG-## data, where ## is the same.
    </sch:p>
    <sch:p xmlns:ism="urn:us:gov:ic:ism" ism:classification="U" ism:ownerProducer="USA" class="codeDesc">
      If the document is an ISM_USGOV_RESOURCE, the current element is the
      ISM_RESOURCE_ELEMENT, and attribute @ism:atomicEnergyMarkings is specified
      with a value containing a token matching [FRD-SG-##], then this rule ensures that some
      element meeting ISM_CONTRIBUTES specifies attribute @ism:atomicEnergyMarkings
      with a value containing a token matching the same [FRD-SG-##].
    </sch:p>
    <sch:rule id="ISM-ID-00230-R1" context="*[$ISM_USGOV_RESOURCE and generate-id(.) = generate-id($ISM_RESOURCE_ELEMENT)]">
        <sch:let name="matchingTokens" value="for $token in tokenize(normalize-space(string(@ism:atomicEnergyMarkings)), ' ') return if(matches($token,'^FRD-SG-[1-9][0-9]?$')) then $token else null"/>  
        <sch:assert test="every $token in $matchingTokens satisfies index-of($partAtomicEnergyMarkings_tok, $token) &gt; 0" flag="error" role="error">
            [ISM-ID-00230][Error] If ISM_USGOV_RESOURCE and attribute @ism:atomicEnergyMarkings of ISM_RESOURCE_ELEMENT contains 
            [FRD-SG-##] then at least one element meeting ISM_CONTRIBUTES in the document must have a 
            @ism:atomicEnergyMarking attribute containing the same [FRD-SG-##].
            
            Human Readable: USA documents marked FRD-SG-## at the resource level must have FRD-SG-## data, where ## is the same.
        </sch:assert>
    </sch:rule>
</sch:pattern>