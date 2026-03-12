<?xml version="1.0" encoding="UTF-8"?>
<?ICEA pattern?>
<?schematron-phases phaseids="BANNER VALUECHECK"?>
<!-- Notices - Distribution Notice: 
           This document has been approved for Public Release and is available for use without restriction.
       -->
<sch:pattern xmlns:sch="http://purl.oclc.org/dsdl/schematron" id="ISM-ID-00478">
    <sch:p xmlns:ism="urn:us:gov:ic:ism" ism:classification="U" ism:ownerProducer="USA" class="ruleText">
        [ISM-ID-00478][Error] If the document has @ism:compliesWith containing the token [USA-CUI], 
        then @ism:compliesWith cannot contain [USIC], because CUI has not yet been implemented in the IC.
        
        Human Readable: A document that contains CUI cannot be an IC document because CUI has not yet been implemented in the IC.
    </sch:p>
    <sch:p xmlns:ism="urn:us:gov:ic:ism" ism:classification="U" ism:ownerProducer="USA" class="codeDesc">
        If the document is an ISM_USCUI_RESOURCE, then @ism:compliesWith cannot contain [USIC].
    </sch:p>
    <sch:rule id="ISM-ID-00478-R1" context="*[$ISM_USCUI_RESOURCE and generate-id(.) = generate-id($ISM_RESOURCE_ELEMENT)]">
        <sch:assert test="not(contains(@ism:compliesWith,'USIC'))" flag="error" role="error">
            [ISM-ID-00478][Error] If the document has @ism:compliesWith containing the token [USA-CUI], 
            then @ism:compliesWith cannot contain [USIC], because CUI has not yet been implemented in the IC.
            
            Human Readable: A document that contains CUI cannot be an IC document because CUI has not yet been implemented in the IC.
        </sch:assert>
    </sch:rule>
</sch:pattern>