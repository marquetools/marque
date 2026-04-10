<?xml version="1.0" encoding="UTF-8"?>
<?ICEA pattern?>
<?schematron-phases phaseids="ROLLDOWN VALUECHECK"?>
<!-- Notices - Distribution Notice: 
           This document has been approved for Public Release and is available for use without restriction.
       -->
<sch:pattern xmlns:sch="http://purl.oclc.org/dsdl/schematron" id="ISM-ID-00303">
    <sch:p xmlns:ism="urn:us:gov:ic:ism" ism:classification="U" ism:ownerProducer="USA" class="ruleText">
        [ISM-ID-00303][Error] If ISM_USGOV_RESOURCE and the document contains attribute 
        @ism:disseminationControls with name token [OC-USGOV] in the banner, then 
        all [OC] portions must also contain [OC-USGOV].
        
        Human Readable: A USA document with OC-USGOV dissemination in the banner
        must also contain OC-USGOV in any OC portions.
    </sch:p>
    <sch:p xmlns:ism="urn:us:gov:ic:ism" ism:classification="U" ism:ownerProducer="USA" class="codeDesc">
    	If the document is an ISM_USGOV_RESOURCE and the resource element
    	contains attribute @ism:disseminationControls with name token [OC-USGOV], then this rule 
    	ensures that every portion contain name token [OC] also contains name token [OC-USGOV].    	
    </sch:p>
    <sch:rule id="ISM-ID-00303-R1" context="*[$ISM_USGOV_RESOURCE and generate-id(.) = generate-id($ISM_RESOURCE_ELEMENT) and util:containsAnyOfTheTokens(@ism:disseminationControls, ('OC-USGOV'))]">
        <sch:let name="portionsWithOC" value="for $portion in $partTags return if($portion[util:containsAnyOfTheTokens(@ism:disseminationControls, ('OC'))]) then $portion else null"/>  
        <sch:assert test="every $portionWithOC in $portionsWithOC satisfies $portionWithOC[util:containsAnyOfTheTokens(@ism:disseminationControls, 'OC-USGOV')]" flag="error" role="error">
            [ISM-ID-00303][Error] If ISM_USGOV_RESOURCE and the document contains attribute 
            @ism:disseminationControls with name token [OC-USGOV] in the banner, then 
            all [OC] portions must also contain [OC-USGOV].
            
            Human Readable: A USA document with OC-USGOV dissemination in the banner
            must also contain OC-USGOV in any OC portions.
        </sch:assert>
    </sch:rule>
</sch:pattern>