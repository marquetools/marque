<?xml version="1.0" encoding="UTF-8"?>
<?ICEA pattern?>
<?schematron-phases phaseids="ROLLUP VALUECHECK"?>
<!-- Notices - Distribution Notice: 
           This document has been approved for Public Release and is available for use without restriction.
       -->
<sch:pattern xmlns:sch="http://purl.oclc.org/dsdl/schematron" id="ISM-ID-00315">
    <sch:p xmlns:ism="urn:us:gov:ic:ism" ism:classification="U" ism:ownerProducer="USA" class="ruleText">
        [ISM-ID-00315][Error] If classified element meets ISM_CONTRIBUTES and
        attribute @ism:ownerProducer contains the token [NATO], then attribute @ism:declassException must be
        specified with a value of [NATO] or [NATO-AEA] on the resourceElement. 
        
        Human Readable: Any document with non-resource classified elements that contributes to the document's banner 
        roll-up and has NATO Information must also specify a NATO declass exemption on the banner. 
    </sch:p>
    <sch:p xmlns:ism="urn:us:gov:ic:ism" ism:classification="U" ism:ownerProducer="USA" class="codeDesc">
        In a classified document that meets ISM_USGOV_RESOURCE, for each
        element which is not the $ISM_RESOURCE_ELEMENT and meets ISM_CONTRIBUTES and specifies
        attribute @ism:ownerProducer with a value containing the token [NATO], this rule ensures that
        attribute @ism:declassExemption on the resource element is specified with a value containing
        the token [NATO] or [NATO-AEA]. 
    </sch:p>
    <sch:rule id="ISM-ID-00315-R1" context="*[not(generate-id(.) = generate-id($ISM_RESOURCE_ELEMENT)) and util:contributesToRollup(.) and $ISM_USGOV_RESOURCE and not(@ism:classification = 'U') and util:containsAnyTokenMatching(@ism:ownerProducer, ('NATO:?'))]">
        <sch:assert test="util:containsAnyOfTheTokens($ISM_RESOURCE_ELEMENT/@ism:declassException, ('NATO', 'NATO-AEA'))" flag="error" role="error"> 
            [ISM-ID-00315][Error] If classified element meets ISM_CONTRIBUTES and
            attribute @ism:ownerProducer contains the token [NATO], then attribute @ism:declassException must be
            specified with a value of [NATO] or [NATO-AEA] on the resourceElement. 
            
            Human Readable: Any document with non-resource classified elements that contributes to the document's banner 
            roll-up and has NATO Information must also specify a NATO declass exemption on the banner. 
        </sch:assert>
    </sch:rule>
</sch:pattern>