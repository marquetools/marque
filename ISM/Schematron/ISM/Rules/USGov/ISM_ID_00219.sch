<?xml version="1.0" encoding="UTF-8"?>
<?ICEA pattern?>
<?schematron-phases phaseids="ROLLDOWN VALUECHECK"?>
<!-- Notices - Distribution Notice: 
           This document has been approved for Public Release and is available for use without restriction.
       -->
<sch:pattern xmlns:sch="http://purl.oclc.org/dsdl/schematron" id="ISM-ID-00219">
    <sch:p xmlns:ism="urn:us:gov:ic:ism" ism:classification="U" ism:ownerProducer="USA" class="ruleText">
        [ISM-ID-00219][Error] If element meets ISM_CONTRIBUTES and attribute
        @ism:ownerProducer contains the token [FGI], then attribute 
        @ism:FGIsourceProtected must have a value containing the token [FGI].
        
        Human Readable: Any non-resource element that contributes to the 
        document's banner roll-up and has FOREIGN GOVERNMENT INFORMATION (FGI)
        must also specify attribute FGIsourceProtected with token FGI.
    </sch:p>
    <sch:p xmlns:ism="urn:us:gov:ic:ism" ism:classification="U" ism:ownerProducer="USA" class="codeDesc">
        For each element which is not the $ISM_RESOURCE_ELEMENT and meets 
        ISM_CONTRIBUTES and specifies attribute @ism:ownerProducer with a value
        containing the token [FGI], this rule ensures that attribute 
        @ism:FGIsourceProtected is specified with a value containing the
        token [FGI].
    </sch:p>
    <sch:rule id="ISM-ID-00219-R1" context="*[not(generate-id(.) = generate-id($ISM_RESOURCE_ELEMENT)) and util:contributesToRollup(.) and util:containsAnyOfTheTokens(@ism:ownerProducer, ('FGI'))]">
        <sch:assert test="util:containsAnyOfTheTokens(@ism:FGIsourceProtected, ('FGI'))" flag="error" role="error">
            [ISM-ID-00219][Error] If element meets ISM_CONTRIBUTES and attribute
            @ism:ownerProducer contains the token [FGI], then attribute 
            @ism:FGIsourceProtected must have a value containing the token [FGI].
            
            Human Readable: Any non-resource element that contributes to the 
            document's banner roll-up and has FOREIGN GOVERNMENT INFORMATION (FGI)
            must also specify attribute FGIsourceProtected with token FGI.
        </sch:assert>
    </sch:rule>
</sch:pattern>