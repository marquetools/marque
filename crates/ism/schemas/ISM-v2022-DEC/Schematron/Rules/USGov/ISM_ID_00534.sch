<?xml version="1.0" encoding="UTF-8"?>
<?ICEA pattern?>
<?schematron-phases phaseids="BANNER STRUCTURECHECK"?>
<!-- Notices - Distribution Notice: 
           This document has been approved for Public Release and is available for use without restriction.
       -->
<sch:pattern xmlns:sch="http://purl.oclc.org/dsdl/schematron" id="ISM-ID-00534">
    <sch:p xmlns:ism="urn:us:gov:ic:ism" ism:classification="U" ism:ownerProducer="USA" class="ruleText">
        [ISM-ID-00534][Error] All elements with @ism:SARIdentifier token(s) containing a dash (-) (excluding the SAR- prefix) will result in an error when @ism:compliesWith are 
        both DoD and IC.  DoD and IC rules differ on how to render SAP markings containing dashes; therefore, it is not allowed to have SAPs 
        with dashes in a document that complies with both DoD and IC rules.
    </sch:p>
    <sch:p xmlns:ism="urn:us:gov:ic:ism" ism:classification="U" ism:ownerProducer="USA" class="codeDesc">
        Find elements with @ism:SARIdentifier when @ism:compliesWith contains both 'USDOD' and 'USIC' ($ISM_USDOD_RESOURCE and $ISM_USIC_RESOURCE).
        If there is any dash in @ism:SARIdentifier after the SAR- prefix, then ERROR.
    </sch:p>
    <sch:rule id="ISM-ID-00534-R1" context="*[@ism:SARIdentifier and $ISM_USDOD_RESOURCE and $ISM_USIC_RESOURCE]">
        <sch:let name="SARsWithDashes"
            value="for $token in tokenize(normalize-space(string(@ism:SARIdentifier)), ' ') return if (contains(substring-after($token,'SAR-'),'-')) then $token 
            else null"/>
        <sch:assert test="count($SARsWithDashes) = 0"
            flag="error" 
            role="error">
            [ISM-ID-00534][Error] If there are any elements with a dash (-) in @ism:SARIdentifier (excluding the SAR- prefix), then it is an ERROR if
            @ism:compliesWith="USGov USDOD USIC".  This is an ERROR because IC rules state that a dash in @ism:SARIdentifier
            indicates a compartment or subcompartment.  A DoD @ism:SARIdentifier with a dash is just a plain SAP marking 
            containing a dash; DoD SAPs do not have compartments or subcompartments. This means DoD and IC rules differ on 
            how to render SAP markings containing dashes; therefore, it is not allowed to have SAPs with dashes in a document 
            that complies with both DoD and IC rules.
        </sch:assert>
    </sch:rule>
</sch:pattern>