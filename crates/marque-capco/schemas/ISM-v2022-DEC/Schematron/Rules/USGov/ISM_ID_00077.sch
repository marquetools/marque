<?xml version="1.0" encoding="UTF-8"?>
<?ICEA pattern?>
<?schematron-phases phaseids="ROLLUP VALUECHECK"?>
<!-- Notices - Distribution Notice: 
           This document has been approved for Public Release and is available for use without restriction.
       -->
<sch:pattern xmlns:sch="http://purl.oclc.org/dsdl/schematron" id="ISM-ID-00077">
    <sch:p xmlns:ism="urn:us:gov:ic:ism" ism:classification="U" ism:ownerProducer="USA" class="ruleText">
        [ISM-ID-00077][Error] If ISM_USGOV_RESOURCE and any element meeting ISM_CONTRIBUTES in the 
        document has the attribute @ism:atomicEnergyMarkings containing [FRD-SG-##] and the ISM_RESOURCE_ELEMENT
        does not have @ism:atomicEnergyMarkings containing [RD], then the ISM_RESOURCE_ELEMENT must have 
        @ism:atomicEnergyMarkings containing [FRD-SG-##]. ## represent digits 1 through 99 the ## must match.
        
        Human Readable: USA documents having Formerly Restricted SIGMA-## data and not having RD data 
        must have the same Formerly Restricted SIGMA-## Data at the resource level.
    </sch:p>
    <sch:p xmlns:ism="urn:us:gov:ic:ism" ism:classification="U" ism:ownerProducer="USA" class="codeDesc">
        If IC Markings System Register and Manual rules do not apply to the document then the rule does not apply
        and this rule returns true. This rule ensures that no element that does not have attribute @ism:excludeFromRollup 
        set to true has attribute @ism:atomicEnergyMarkings specified with a value containing [FRD-SG-##], 
        where ## is represented by a regular expression matching numbers 1 through 99, unless the resourceElement 
        also has attribute @ism:atomicEnergyMarkings specified with a value containing [FRD-SG-##] or [RD] is specified 
        on the ISM_RESOURCE_ELEMENT.
    </sch:p>
    <sch:rule id="ISM-ID-00077-R1" context="*[$ISM_USGOV_RESOURCE and generate-id(.) = generate-id($ISM_RESOURCE_ELEMENT) and not(util:containsAnyOfTheTokens($ISM_RESOURCE_ELEMENT/@ism:atomicEnergyMarkings, ('RD')))]">
        <sch:let name="matchingTokens" value="for $token in $partAtomicEnergyMarkings_tok return if(matches($token,'^FRD-SG-[1-9][0-9]?$')) then $token else null"/>
      <sch:assert test="every $token in $matchingTokens satisfies index-of($bannerAtomicEnergyMarkings_tok, $token) &gt; 0" flag="error" role="error">
          [ISM-ID-00077][Error] If ISM_USGOV_RESOURCE and any element meeting ISM_CONTRIBUTES in the 
          document has the attribute @ism:atomicEnergyMarkings containing [FRD-SG-##] and the ISM_RESOURCE_ELEMENT
          does not have @ism:atomicEnergyMarkings containing [RD], then the ISM_RESOURCE_ELEMENT must have 
          @ism:atomicEnergyMarkings containing [FRD-SG-##]. ## represent digits 1 through 99 the ## must match.
          
          Human Readable: USA documents having Formerly Restricted SIGMA-## data and not having RD data 
          must have the same Formerly Restricted SIGMA-## Data at the resource level.
        </sch:assert>
    </sch:rule>
</sch:pattern>